//! Narrow extension surfaces; no host filesystem/shell bridge to JavaScript.
use crate::{
    browser::{clear, label, Browser, Tab},
    extensions::{self, Action, Installed},
};
use gtk::{gio, glib, prelude::*};
use std::{
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};
use webkit::prelude::*;

impl Browser {
    pub fn refresh_extensions(self: &Rc<Self>) {
        if self.safe_mode {
            return;
        }
        let fingerprint = extensions::fingerprint();
        if fingerprint == *self.extension_fingerprint.borrow() { return; }
        *self.extension_fingerprint.borrow_mut() = fingerprint;
        let enabled: Vec<_> = extensions::list()
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.enabled)
            .collect();
        let previous: Vec<_> = self
            .extensions
            .borrow()
            .iter()
            .map(|e| (e.manifest.id.clone(), e.digest.clone()))
            .collect();
        let current: Vec<_> = enabled
            .iter()
            .map(|e| (e.manifest.id.clone(), e.digest.clone()))
            .collect();
        if current == previous {
            return;
        }
        let revoked: Vec<_> = self
            .extensions
            .borrow()
            .iter()
            .filter(|e| !current.contains(&(e.manifest.id.clone(), e.digest.clone())))
            .cloned()
            .collect();
        for extension in &revoked {
            for (id, weak) in self.extension_views.borrow().iter() {
                if id == &extension.manifest.id {
                    if let Some(view) = weak.upgrade() {
                        view.terminate_web_process();
                    }
                }
            }
            for tab in self.tabs.borrow().iter().filter(|t| !t.private) {
                let origin = crate::core::origin(&tab.page.borrow().url).unwrap_or_default();
                if extension.manifest.sites.iter().any(|s| s.origin == origin) {
                    if let Some(view) = tab.view.borrow().as_ref() {
                        view.terminate_web_process();
                    }
                }
            }
        }
        self.extension_views
            .borrow_mut()
            .retain(|(_, v)| v.upgrade().is_some());
        *self.extensions.borrow_mut() = enabled;
        if !revoked.is_empty() {
            self.notice("Extension access changed. Affected pages were stopped; reload them to continue without the old extension.");
        }
    }
    pub fn approve_extension(
        self: &Rc<Self>,
        id: &str,
        done: impl FnOnce(Result<(), String>) + 'static,
    ) {
        if self.safe_mode {
            done(Err("Extensions are disabled in safe mode".into()));
            return;
        }
        let extension = match extensions::load(id) {
            Ok(e) => e,
            Err(e) => {
                done(Err(e));
                return;
            }
        };
        let m = &extension.manifest;
        let sites = m
            .sites
            .iter()
            .map(|s| s.origin.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let dialog = gtk::AlertDialog::builder().message(format!("Enable {}?", m.name))
            .detail(format!("{}\n{} commands; sidebar: {}; new tab: {}.\nSite hooks may read/change signed-in pages and make network requests on:\n{}\nCommands can open URLs or change settings when invoked. Panels have no network or native bridge. Only enable code you trust. Changes require approval again.", m.description, m.commands.len(), !m.sidebar_html.is_empty(), !m.new_tab_html.is_empty(), if sites.is_empty() { "No site access" } else { &sites }))
            .buttons(["Cancel", "Enable"]).cancel_button(0).default_button(0).build();
        let weak = Rc::downgrade(self);
        dialog.choose(Some(&self.window), gio::Cancellable::NONE, move |result| {
            let Some(b) = weak.upgrade() else {
                done(Err("Browser closed".into()));
                return;
            };
            if result != Ok(1) {
                done(Err("Extension approval cancelled".into()));
                return;
            }
            let result = extensions::grant(&extension.manifest.id, Some(&extension.digest));
            if result.is_ok() {
                b.refresh_extensions();
            }
            done(result);
        });
    }
    pub fn extension_command(
        self: &Rc<Self>,
        extension: &str,
        command: &str,
    ) -> Result<(), String> {
        if self.safe_mode {
            return Err("Extensions are disabled in safe mode".into());
        }
        let installed = extensions::load(extension)?;
        if !installed.enabled {
            return Err("Extension is not approved at this revision".into());
        }
        let command = installed
            .manifest
            .commands
            .iter()
            .find(|c| c.id == command)
            .ok_or("Unknown extension command")?;
        match &command.action {
            Action::Open { url } => {
                self.new_tab(url, false, true);
            }
            Action::Sidebar => self.show_extension_sidebar(&installed),
            Action::Configure { changes } => {
                let initial = crate::config::document()?;
                crate::config_store::transact(
                    &crate::config::path(),
                    &initial.settings,
                    None,
                    false,
                    |d| {
                        for (key, value) in changes {
                            crate::config::set(
                                &mut d.settings,
                                key,
                                &value
                                    .as_str()
                                    .map(str::to_owned)
                                    .unwrap_or_else(|| value.to_string()),
                            )?;
                        }
                        Ok(())
                    },
                )?;
            }
        }
        Ok(())
    }
    pub fn show_extensions(self: &Rc<Self>) {
        clear(&self.panel);
        *self.panel_kind.borrow_mut() = "Extensions".into();
        self.panel.set_visible(true);
        self.panel.append(&label("Personal extensions", "heading"));
        let close = gtk::Button::with_label("Close");
        let weak = Rc::downgrade(self);
        close.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.panel.set_visible(false);
                clear(&b.panel);
            }
        });
        self.panel.append(&close);
        if self.safe_mode {
            self.panel.append(&label("Disabled in safe mode", ""));
            return;
        }
        for result in extensions::list() {
            let extension = match result {
                Ok(e) => e,
                Err(e) => {
                    self.panel.append(&label(&e, ""));
                    continue;
                }
            };
            self.panel
                .append(&label(&extension.manifest.name, "heading"));
            let description = label(&extension.manifest.description, "muted");
            description.set_wrap(true);
            self.panel.append(&description);
            let button = gtk::Button::with_label(if extension.enabled {
                "Disable"
            } else {
                "Review and enable"
            });
            let weak = Rc::downgrade(self);
            let id = extension.manifest.id.clone();
            let enabled = extension.enabled;
            button.connect_clicked(move |_| {
                if let Some(b) = weak.upgrade() {
                    if enabled {
                        if let Err(e) = extensions::grant(&id, None) {
                            b.notice(&e);
                        }
                        b.refresh_extensions();
                        b.show_extensions();
                    } else {
                        let weak = Rc::downgrade(&b);
                        b.approve_extension(&id, move |r| {
                            if let Some(b) = weak.upgrade() {
                                if let Err(e) = r {
                                    b.notice(&e);
                                }
                                b.show_extensions();
                            }
                        });
                    }
                }
            });
            self.panel.append(&button);
            if extension.enabled {
                for command in &extension.manifest.commands {
                    let button = gtk::Button::with_label(&command.label);
                    let weak = Rc::downgrade(self);
                    let id = extension.manifest.id.clone();
                    let command = command.id.clone();
                    button.connect_clicked(move |_| {
                        if let Some(b) = weak.upgrade() {
                            if let Err(e) = b.extension_command(&id, &command) {
                                b.notice(&e);
                            }
                        }
                    });
                    self.panel.append(&button);
                }
            }
        }
    }
    fn show_extension_sidebar(self: &Rc<Self>, extension: &Installed) {
        clear(&self.panel);
        *self.panel_kind.borrow_mut() = format!("Extension:{}", extension.manifest.id);
        self.panel
            .append(&label(&extension.manifest.name, "heading"));
        let view = self.extension_view(extension, &extension.manifest.sidebar_html);
        self.panel.append(&view);
        self.panel.set_visible(true);
    }
    pub fn extension_view(self: &Rc<Self>, extension: &Installed, html: &str) -> webkit::WebView {
        let settings = webkit::Settings::new();
        settings.set_javascript_can_open_windows_automatically(false);
        let view = webkit::WebView::builder()
            .settings(&settings)
            .network_session(&webkit::NetworkSession::new_ephemeral())
            .hexpand(true)
            .vexpand(true)
            .build();
        view.connect_permission_request(|_, request| {
            request.deny();
            true
        });
        view.connect_decide_policy(|_, decision, kind| {
            if kind == webkit::PolicyDecisionType::NewWindowAction {
                decision.ignore();
                return true;
            }
            if let Some(nav) = decision.downcast_ref::<webkit::NavigationPolicyDecision>() {
                let uri = nav
                    .navigation_action()
                    .and_then(|a| a.request())
                    .and_then(|r| r.uri());
                if uri.as_deref() != Some("about:blank") {
                    decision.ignore();
                    return true;
                }
            }
            false
        });
        let content = format!("<!doctype html><html><head><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; connect-src 'none'; frame-src 'none'; form-action 'none'; base-uri 'none'\"></head><body>{html}</body></html>");
        view.load_html(&content, Some("about:blank"));
        self.extension_views
            .borrow_mut()
            .retain(|(_, v)| v.upgrade().is_some());
        self.extension_views
            .borrow_mut()
            .push((extension.manifest.id.clone(), view.downgrade()));
        self.watch_extension(&view, &extension.manifest.id, None);
        view
    }
    fn watch_extension(
        self: &Rc<Self>,
        view: &webkit::WebView,
        id: &str,
        generation: Option<(std::rc::Weak<Tab>, u64)>,
    ) {
        let weak = Rc::downgrade(self);
        let view = view.downgrade();
        let id = id.to_owned();
        let heartbeat = Rc::new(Cell::new(Instant::now()));
        let waiting = Rc::new(Cell::new(false));
        glib::timeout_add_local(Duration::from_millis(500), move || {
            let (Some(b), Some(view)) = (weak.upgrade(), view.upgrade()) else {
                return glib::ControlFlow::Break;
            };
            if generation
                .as_ref()
                .is_some_and(|(tab, g)| tab.upgrade().is_none_or(|t| t.generation.get() != *g))
            {
                return glib::ControlFlow::Break;
            }
            if view.parent().is_none() || b.closing.get() {
                return glib::ControlFlow::Break;
            }
            if !b.extensions.borrow().iter().any(|e| e.manifest.id == id) {
                return glib::ControlFlow::Break;
            }
            if heartbeat.get().elapsed() > Duration::from_secs(3) {
                view.terminate_web_process();
                let _ = extensions::grant(&id, None);
                b.refresh_extensions();
                b.notice("An extension stopped responding and was disabled. Reload the affected page to recover.");
                return glib::ControlFlow::Break;
            }
            if !waiting.replace(true) {
                let heartbeat = heartbeat.clone();
                let waiting = waiting.clone();
                view.evaluate_javascript(
                    "true",
                    Some("nagi-extension-watchdog"),
                    None,
                    gio::Cancellable::NONE,
                    move |_| {
                        heartbeat.set(Instant::now());
                        waiting.set(false);
                    },
                );
            }
            glib::ControlFlow::Continue
        });
    }
    pub fn extension_navigation(self: &Rc<Self>, tab: &Rc<Tab>) {
        if self.safe_mode || tab.private || tab.failed.get() {
            return;
        }
        let Some(view) = tab.view.borrow().clone() else {
            return;
        };
        let Some(origin) = view.uri().as_deref().and_then(crate::core::origin) else {
            return;
        };
        let installed = self.extensions.borrow().clone();
        for extension in installed {
            if !extensions::load(&extension.manifest.id)
                .is_ok_and(|e| e.enabled && e.digest == extension.digest)
            {
                continue;
            }
            for site in extension
                .manifest
                .sites
                .iter()
                .filter(|s| s.origin == origin)
            {
                let script = format!("(() => {{ if(location.origin !== {}) return; const s=document.createElement('style'); s.textContent={}; document.documentElement.append(s); {} }})();", serde_json::to_string(&site.origin).unwrap(), serde_json::to_string(&site.css).unwrap(), site.script);
                let weak = Rc::downgrade(self);
                let id = extension.manifest.id.clone();
                view.evaluate_javascript(
                    &script,
                    Some(&format!("nagi-extension-{}", extension.manifest.id)),
                    None,
                    gio::Cancellable::NONE,
                    move |result| {
                        if let Some(b) = weak.upgrade() {
                            if result.is_err() {
                                let _ = extensions::grant(&id, None);
                                b.refresh_extensions();
                                b.notice(
                                    "A site extension failed and was disabled; reload the page.",
                                );
                            }
                        }
                    },
                );
                self.watch_extension(
                    &view,
                    &extension.manifest.id,
                    Some((Rc::downgrade(tab), tab.generation.get())),
                );
            }
        }
        self.extension_event("navigation.finished", &origin);
    }
    pub fn extension_event(&self, kind: &str, origin: &str) {
        if self.safe_mode {
            return;
        }
        for installed in self.extensions.borrow().iter() {
            if !extensions::load(&installed.manifest.id)
                .is_ok_and(|e| e.enabled && e.digest == installed.digest)
            {
                continue;
            }
            for event in &installed.manifest.events {
                if event.kind == kind && event.origin == origin {
                    self.notice(&event.message);
                }
            }
        }
    }
}
