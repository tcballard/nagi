use crate::{
    browser::{clear, Browser, DownloadRow, Tab},
    core::*,
};
use gtk::{gio, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use webkit::prelude::*;

impl Browser {
    pub fn load(&self, v: &webkit::WebView, uri: &str) {
        if self.filter_ready.get() {
            v.load_uri(uri);
        } else {
            self.pending.borrow_mut().push((v.downgrade(), uri.into()));
        }
    }
    pub fn build_view(
        self: &Rc<Self>,
        tab: &Rc<Tab>,
        related: Option<&webkit::WebView>,
    ) -> webkit::WebView {
        clear(&tab.holder);
        let manager = webkit::UserContentManager::new();
        let settings = webkit::Settings::new();
        settings.set_enable_developer_extras(true);
        settings.set_enable_hyperlink_auditing(false);
        settings.set_javascript_can_open_windows_automatically(false);
        settings.set_media_playback_requires_user_gesture(true);
        settings.set_enable_fullscreen(true);
        let mut builder = webkit::WebView::builder()
            .user_content_manager(&manager)
            .settings(&settings)
            .hexpand(true)
            .vexpand(true)
            .zoom_level(self.state.borrow().settings.zoom);
        if let Some(v) = related {
            builder = builder.related_view(v);
        } else {
            let session = if tab.private {
                let s = webkit::NetworkSession::new_ephemeral();
                self.setup_downloads(&s);
                s
            } else {
                self.session.clone()
            };
            builder = builder.network_session(&session);
        }
        let view = builder.build();
        *tab.view.borrow_mut() = Some(view.clone());
        tab.holder.append(&view);
        self.refresh_content(tab);
        let weak = Rc::downgrade(self);
        let wt = Rc::downgrade(tab);
        view.connect_load_changed(move |v, event| {
            let (Some(b), Some(t)) = (weak.upgrade(), wt.upgrade()) else {
                return;
            };
            if event == webkit::LoadEvent::Started {
                t.failed.set(false);
                t.picking.set(false);
            }
            if !t.reader.get() {
                if let Some(uri) = v.uri() {
                    if uri != "about:blank" {
                        t.page.borrow_mut().url = uri.to_string();
                    }
                }
                if let Some(title) = v.title() {
                    if !title.is_empty() {
                        t.page.borrow_mut().title = title.to_string();
                    }
                }
            }
            if event == webkit::LoadEvent::Finished
                && !t.private
                && !t.failed.get()
                && !t.reader.get()
            {
                let p = t.page.borrow();
                b.state.borrow_mut().visit(&p.url, &p.title);
            }
            b.dirty.set(true);
            b.update_chrome();
        });
        let weak = Rc::downgrade(self);
        let wt = Rc::downgrade(tab);
        view.connect_title_notify(move |v| {
            if let (Some(b), Some(t)) = (weak.upgrade(), wt.upgrade()) {
                if !t.reader.get() {
                    if let Some(title) = v.title() {
                        t.page.borrow_mut().title = title.to_string();
                    }
                }
                b.dirty.set(true);
                b.update_chrome();
            }
        });
        let weak = Rc::downgrade(self);
        view.connect_estimated_load_progress_notify(move |_| {
            if let Some(b) = weak.upgrade() {
                b.update_chrome();
            }
        });
        let weak = Rc::downgrade(self);
        let wt = Rc::downgrade(tab);
        view.connect_load_failed(move |_, _, uri, error| {
            if error.matches(webkit::NetworkError::Cancelled) {
                return true;
            }
            if let (Some(b), Some(t)) = (weak.upgrade(), wt.upgrade()) {
                t.failed.set(true);
                b.notice(&format!(
                    "Could not open {}: {}",
                    host(uri),
                    error.message()
                ));
                b.update_chrome();
            }
            false
        });
        let weak = Rc::downgrade(self);
        let wt = Rc::downgrade(tab);
        view.connect_load_failed_with_tls_errors(move |_, uri, _, _| {
            if let (Some(b), Some(t)) = (weak.upgrade(), wt.upgrade()) {
                t.failed.set(true);
                b.notice(&format!(
                    "The certificate for {} could not be verified. The connection was blocked.",
                    host(uri)
                ));
                b.update_chrome();
            }
            false
        });
        let weak = Rc::downgrade(self);
        view.connect_web_process_terminated(move |_, _| {
            if let Some(b) = weak.upgrade() {
                b.notice("This page stopped responding. Reload it to continue.");
            }
        });
        let weak = Rc::downgrade(self);
        let id = tab.id;
        view.connect_mouse_target_changed(move |_, hit, _| {
            if let Some(b) = weak.upgrade() {
                if b.active.get() == id {
                    b.status.set_text(hit.link_uri().as_deref().unwrap_or(""));
                }
            }
        });
        let weak = Rc::downgrade(self);
        view.connect_enter_fullscreen(move |_| {
            if let Some(b) = weak.upgrade() {
                b.window.fullscreen();
                b.chrome.set_visible(false);
            }
            false
        });
        let weak = Rc::downgrade(self);
        view.connect_leave_fullscreen(move |_| {
            if let Some(b) = weak.upgrade() {
                b.window.unfullscreen();
                b.chrome.set_visible(true);
            }
            false
        });
        let weak = Rc::downgrade(self);
        let private = tab.private;
        view.connect_create(move |v, _| {
            if let Some(b) = weak.upgrade() {
                let t = b.new_tab("about:blank", private, false);
                let child = b.build_view(&t, Some(v));
                b.select(t.id);
                child.upcast()
            } else {
                webkit::WebView::new().upcast()
            }
        });
        let weak = Rc::downgrade(self);
        let id = tab.id;
        view.connect_close(move |_| {
            if let Some(b) = weak.upgrade() {
                b.close_tab(id);
            }
        });
        let weak = Rc::downgrade(self);
        view.connect_decide_policy(move |_, decision, kind| {
            if kind == webkit::PolicyDecisionType::Response {
                if let Some(d) = decision.downcast_ref::<webkit::ResponsePolicyDecision>() {
                    if !d.is_mime_type_supported() {
                        d.download();
                        return true;
                    }
                }
            }
            if let Some(d) = decision.downcast_ref::<webkit::NavigationPolicyDecision>() {
                if let Some(mut action) = d.navigation_action() {
                    if let Some(uri) = action.request().and_then(|r| r.uri()) {
                        if let Ok(url) = url::Url::parse(&uri) {
                            if !["http", "https", "file", "about", "blob", "data"]
                                .contains(&url.scheme())
                            {
                                d.ignore();
                                if action.is_user_gesture() {
                                    if let Some(b) = weak.upgrade() {
                                        b.external(&uri);
                                    }
                                }
                                return true;
                            }
                        }
                    }
                }
            }
            false
        });
        let weak = Rc::downgrade(self);
        view.connect_permission_request(move |v, request| {
            let Some(b) = weak.upgrade() else {
                request.deny();
                return true;
            };
            let description = if request.is::<webkit::UserMediaPermissionRequest>() {
                "use your camera or microphone"
            } else if request.is::<webkit::GeolocationPermissionRequest>() {
                "access your location"
            } else if request.is::<webkit::NotificationPermissionRequest>() {
                "show notifications"
            } else {
                request.deny();
                b.notice("This page requested a permission that Nagi does not support yet.");
                return true;
            };
            let uri = v.uri().unwrap_or_default().to_string();
            let original = origin(&uri);
            let request = request.clone();
            let view = v.downgrade();
            let dialog = gtk::AlertDialog::builder()
                .message(format!(
                    "Allow {} to {description}?",
                    original.as_deref().unwrap_or("this page")
                ))
                .detail("Permission applies to this request only.")
                .buttons(["Deny", "Allow"])
                .cancel_button(0)
                .default_button(0)
                .build();
            dialog.choose(Some(&b.window), gio::Cancellable::NONE, move |result| {
                if result == Ok(1)
                    && view
                        .upgrade()
                        .and_then(|v| v.uri())
                        .and_then(|u| origin(&u))
                        == original
                {
                    request.allow();
                } else {
                    request.deny();
                }
            });
            true
        });
        // Isolated-world channel: websites cannot manufacture persistent hide rules.
        manager.register_script_message_handler("nagiHide", Some("nagi"));
        let weak = Rc::downgrade(self);
        let wt = Rc::downgrade(tab);
        manager.connect_script_message_received(Some("nagiHide"), move |_, value| {
            let (Some(b), Some(t)) = (weak.upgrade(), wt.upgrade()) else {
                return;
            };
            if !t.picking.replace(false) {
                return;
            }
            let Ok(payload) = serde_json::from_str::<serde_json::Value>(&value.to_str()) else {
                return;
            };
            let selector = payload["selector"].as_str().unwrap_or("");
            let site = payload["origin"].as_str().unwrap_or("");
            if selector.is_empty()
                || selector.len() > 2000
                || selector.contains(['{', '}'])
                || Some(site.to_string()) != origin(&t.page.borrow().url)
            {
                return;
            }
            if !t.private {
                let mut state = b.state.borrow_mut();
                let rules = state.hidden.entry(site.into()).or_default();
                if rules.len() < 100 && !rules.iter().any(|s| s == selector) {
                    rules.push(selector.into());
                }
                drop(state);
                b.dirty.set(true);
                b.refresh_content(&t);
            }
            b.notice(if t.private {
                "Element hidden for this private page only."
            } else {
                "Element hidden. Restore it from Site protection → Restore hidden elements."
            });
        });
        view
    }
    pub fn external(self: &Rc<Self>, uri: &str) {
        let dialog = gtk::AlertDialog::builder()
            .message("Open another application?")
            .detail(uri)
            .buttons(["Cancel", "Open"])
            .cancel_button(0)
            .default_button(0)
            .build();
        let uri = uri.to_owned();
        let parent = self.window.clone();
        dialog.choose(Some(&self.window), gio::Cancellable::NONE, move |result| {
            if result == Ok(1) {
                gtk::UriLauncher::new(&uri).launch(Some(&parent), gio::Cancellable::NONE, |_| {});
            }
        });
    }
    pub fn refresh_content(&self, tab: &Tab) {
        let Some(v) = tab.view.borrow().clone() else {
            return;
        };
        let Some(m) = v.user_content_manager() else {
            return;
        };
        m.remove_all_filters();
        if let Some(filter) = self.filter.borrow().as_ref() {
            m.add_filter(filter);
        }
        m.remove_all_scripts();
        let hidden = serde_json::to_string(&self.state.borrow().hidden).unwrap_or_default();
        let js = format!(
            r#"(()=>{{const rules={hidden};const selectors=rules[location.origin]||[];if(!selectors.length)return;const apply=()=>{{if(!document.documentElement)return false;const style=document.createElement('style');style.textContent=selectors.map(s=>s+'{{display:none!important}}').join('\n');document.documentElement.append(style);return true;}};if(!apply()){{const observer=new MutationObserver(()=>{{if(apply())observer.disconnect();}});observer.observe(document,{{childList:true,subtree:true}});}}}})();"#
        );
        m.add_script(&webkit::UserScript::for_world(
            &js,
            webkit::UserContentInjectedFrames::TopFrame,
            webkit::UserScriptInjectionTime::Start,
            "nagi",
            &[],
            &[],
        ));
    }
    pub fn compile_filter(self: &Rc<Self>) {
        let generation = self.filter_generation.get() + 1;
        self.filter_generation.set(generation);
        let state = self.state.borrow();
        let mut rules = vec![];
        if state.settings.block {
            for domain in [
                "doubleclick.net",
                "googlesyndication.com",
                "googleadservices.com",
                "google-analytics.com",
                "googletagmanager.com",
                "adnxs.com",
                "adsrvr.org",
                "scorecardresearch.com",
                "quantserve.com",
                "criteo.com",
                "criteo.net",
                "taboola.com",
                "outbrain.com",
                "amazon-adsystem.com",
                "adroll.com",
                "hotjar.com",
                "hotjar.io",
                "clarity.ms",
                "segment.io",
                "segment.com",
            ] {
                rules.push(serde_json::json!({"trigger":{"url-filter":format!("^https?://([^/]+\\.)?{}/",domain.replace('.',"\\.")),"load-type":["third-party"]},"action":{"type":"block"}}));
            }
            for site in &state.allowed {
                rules.push(serde_json::json!({"trigger":{"url-filter":".*","if-top-url":[format!("^{}/",site.replace('.',"\\."))]},"action":{"type":"ignore-previous-rules"}}));
            }
        }
        // WebKit accepts an empty list; no silent fallback to uncompiled rules.
        let bytes = glib::Bytes::from_owned(serde_json::to_vec(&rules).unwrap());
        drop(state);
        let store =
            webkit::UserContentFilterStore::new(&cache_dir().join("filters").to_string_lossy());
        let weak = Rc::downgrade(self);
        store.save(
            "nagi-basic-v1",
            &bytes,
            gio::Cancellable::NONE,
            move |result| {
                if let Some(b) = weak.upgrade() {
                    if b.filter_generation.get() != generation {
                        return;
                    }
                    match result {
                        Ok(filter) => {
                            *b.filter.borrow_mut() = Some(filter);
                            for tab in b.tabs.borrow().iter() {
                                b.refresh_content(tab);
                            }
                        }
                        Err(e) => b.notice(&format!("Content protection is unavailable: {e}")),
                    };
                    b.filter_ready.set(true);
                    let pending = std::mem::take(&mut *b.pending.borrow_mut());
                    for (v, uri) in pending {
                        if let Some(v) = v.upgrade() {
                            v.load_uri(&uri);
                        }
                    }
                }
            },
        );
    }
    pub fn reader(self: &Rc<Self>) {
        let Some(tab) = self.tab() else { return };
        let Some(v) = self.view() else { return };
        if tab.reader.replace(false) {
            let uri = tab.page.borrow().url.clone();
            v.load_uri(&uri);
            return;
        }
        let weak = Rc::downgrade(self);
        let wt = Rc::downgrade(&tab);
        let source = tab.page.borrow().url.clone();
        v.evaluate_javascript(r#"(()=>{const a=document.querySelector('article')||document.querySelector('main')||document.body;if(!a)return '';const c=a.cloneNode(true);c.querySelectorAll('nav,aside,footer,header,script,style,form,button,[role="navigation"]').forEach(e=>e.remove());return JSON.stringify({title:document.title,text:c.textContent});})()"#,Some("nagi"),None,gio::Cancellable::NONE,move |result|{
            let (Some(b),Some(t))=(weak.upgrade(),wt.upgrade())else{return};if t.page.borrow().url!=source{return;}
            if let Ok(value)=result{if let Ok(article)=serde_json::from_str::<serde_json::Value>(&value.to_str()){
                let text=article["text"].as_str().unwrap_or("");if text.trim().len()<100{b.notice("There is not enough article text on this page for reading view.");return;}
                let p=b.palette.borrow();let content=format!("<!doctype html><meta charset='utf-8'><meta http-equiv='Content-Security-Policy' content=\"default-src 'none'; style-src 'unsafe-inline'\"><style>body{{max-width:740px;margin:60px auto;padding:0 30px;background:{};color:{};font:20px/1.8 Georgia,serif}}h1{{font:38px/1.2 sans-serif}}p{{white-space:pre-wrap}}small{{font:13px sans-serif;opacity:.65}}</style><small>READING VIEW · Ctrl+Shift+R to return</small><h1>{}</h1><p>{}</p>",p.bg,p.fg,html(article["title"].as_str().unwrap_or("Article")),html(text.trim()));t.reader.set(true);if let Some(v)=t.view.borrow().as_ref(){v.load_html(&content,None);}
            }}
        });
    }
    pub fn hide_element(self: &Rc<Self>) {
        let Some(t) = self.tab() else { return };
        let Some(v) = self.view() else { return };
        t.picking.set(true);
        self.notice("Click an element to hide it. Press Escape to cancel.");
        v.evaluate_javascript(
            include_str!("../assets/hide.js"),
            Some("nagi"),
            None,
            gio::Cancellable::NONE,
            |_| {},
        );
    }
    pub fn setup_downloads(self: &Rc<Self>, session: &webkit::NetworkSession) {
        let weak = Rc::downgrade(self);
        session.connect_download_started(move |_, download| {
            let Some(b) = weak.upgrade() else {
                download.cancel();
                return;
            };
            let item = Rc::new(DownloadRow {
                download: download.clone(),
                title: RefCell::new("Download".into()),
                status: RefCell::new("Choosing destination…".into()),
                done: Cell::new(false),
                failed: Cell::new(false),
            });
            b.downloads.borrow_mut().push(item.clone());
            let weak = Rc::downgrade(&b);
            let wi = Rc::downgrade(&item);
            download.connect_decide_destination(move |d, name| {
                let (Some(b), Some(item)) = (weak.upgrade(), wi.upgrade()) else {
                    d.cancel();
                    return true;
                };
                *item.title.borrow_mut() = safe_filename(name);
                let dialog = gtk::FileDialog::builder()
                    .title("Save download")
                    .initial_name(safe_filename(name))
                    .build();
                let d = d.clone();
                let weak = Rc::downgrade(&b);
                dialog.save(Some(&b.window), gio::Cancellable::NONE, move |result| {
                    match result {
                        Ok(file) => {
                            if let Some(path) = file.path() {
                                d.set_allow_overwrite(true);
                                d.set_destination(&path.to_string_lossy());
                            } else {
                                d.cancel();
                            }
                        }
                        Err(_) => d.cancel(),
                    };
                    if let Some(b) = weak.upgrade() {
                        b.show_panel("Downloads");
                    }
                });
                true
            });
            let wi = Rc::downgrade(&item);
            download.connect_received_data(move |d, _| {
                if let Some(i) = wi.upgrade() {
                    *i.status.borrow_mut() = format!(
                        "{:.0}% · {:.1} MB",
                        d.estimated_progress() * 100.0,
                        d.received_data_length() as f64 / 1048576.0
                    );
                }
            });
            let wi = Rc::downgrade(&item);
            download.connect_failed(move |_, e| {
                if let Some(i) = wi.upgrade() {
                    i.failed.set(true);
                    *i.status.borrow_mut() = format!("Stopped: {}", e.message());
                }
            });
            let wi = Rc::downgrade(&item);
            let weak = Rc::downgrade(&b);
            download.connect_finished(move |_| {
                if let Some(i) = wi.upgrade() {
                    i.done.set(true);
                    if !i.failed.get() {
                        *i.status.borrow_mut() = "Complete".into();
                    }
                }
                if let Some(b) = weak.upgrade() {
                    if *b.panel_kind.borrow() == "Downloads" && b.panel.is_visible() {
                        b.show_panel("Downloads");
                    }
                }
            });
            let weak = Rc::downgrade(&b);
            let wi = Rc::downgrade(&item);
            glib::timeout_add_local(std::time::Duration::from_secs(1), move || {
                let (Some(b), Some(i)) = (weak.upgrade(), wi.upgrade()) else {
                    return glib::ControlFlow::Break;
                };
                if i.done.get() {
                    return glib::ControlFlow::Break;
                }
                if *b.panel_kind.borrow() == "Downloads" && b.panel.is_visible() {
                    b.show_panel("Downloads");
                }
                glib::ControlFlow::Continue
            });
        });
    }
}
