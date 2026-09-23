use crate::{
    browser::{clear, icon, label, Browser},
    core::*,
};
use gtk::{gio, prelude::*};
use std::rc::Rc;
use webkit::prelude::*;
impl Browser {
    pub fn bookmark(self: &Rc<Self>) {
        let Some(t) = self.tab() else { return };
        let page = t.page.borrow().clone();
        if !page.url.starts_with("http") {
            return;
        }
        let mut s = self.state.borrow_mut();
        if let Some(i) = s.bookmarks.iter().position(|p| p.url == page.url) {
            s.bookmarks.remove(i);
        } else {
            s.bookmarks.push(page);
        }
        drop(s);
        self.dirty.set(true);
        self.update_chrome();
        if *self.panel_kind.borrow() == "Bookmarks" {
            self.show_panel("Bookmarks");
        }
    }
    pub fn show_panel(self: &Rc<Self>, kind: &str) {
        *self.panel_kind.borrow_mut() = kind.into();
        clear(&self.panel);
        self.panel.set_visible(true);
        let heading = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let title = label(kind, "heading");
        title.set_xalign(0.0);
        title.set_hexpand(true);
        heading.append(&title);
        let close = icon("window-close-symbolic", "Close panel");
        heading.append(&close);
        self.panel.append(&heading);
        let weak = Rc::downgrade(self);
        close.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.panel.set_visible(false);
            }
        });
        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&content)
            .build();
        match kind {
            "Tabs" | "History" | "Bookmarks" => {
                let search = gtk::SearchEntry::new();
                search.set_placeholder_text(Some("Filter by title or address"));
                self.panel.append(&search);
                let list = gtk::ListBox::new();
                list.set_selection_mode(gtk::SelectionMode::None);
                content.append(&list);
                let render = {
                    let weak = Rc::downgrade(self);
                    let list = list.clone();
                    let kind = kind.to_owned();
                    move |query: &str| {
                        if let Some(b) = weak.upgrade() {
                            while let Some(c) = list.first_child() {
                                list.remove(&c);
                            }
                            let q = query.to_lowercase();
                            let entries: Vec<(Page, Option<u64>)> = match kind.as_str() {
                                "Tabs" => b
                                    .tabs
                                    .borrow()
                                    .iter()
                                    .map(|t| (t.page.borrow().clone(), Some(t.id)))
                                    .collect(),
                                "Bookmarks" => b
                                    .state
                                    .borrow()
                                    .bookmarks
                                    .iter()
                                    .cloned()
                                    .map(|p| (p, None))
                                    .collect(),
                                _ => b
                                    .state
                                    .borrow()
                                    .history
                                    .iter()
                                    .map(|v| {
                                        (
                                            Page {
                                                url: v.url.clone(),
                                                title: v.title.clone(),
                                                pinned: false,
                                            },
                                            None,
                                        )
                                    })
                                    .collect(),
                            };
                            let mut count = 0;
                            for (page, id) in entries {
                                if !(page.title.to_lowercase().contains(&q)
                                    || page.url.to_lowercase().contains(&q))
                                {
                                    continue;
                                }
                                if count >= 150 {
                                    break;
                                }
                                count += 1;
                                let row = gtk::Box::new(gtk::Orientation::Horizontal, 5);
                                let btn = gtk::Button::new();
                                btn.add_css_class("flat");
                                btn.set_hexpand(true);
                                let text = gtk::Box::new(gtk::Orientation::Vertical, 4);
                                let title = label(&page.title, "");
                                title.set_xalign(0.0);
                                title.set_ellipsize(gtk::pango::EllipsizeMode::End);
                                title.set_max_width_chars(30);
                                let url = label(&page.url, "muted");
                                url.set_xalign(0.0);
                                url.set_ellipsize(gtk::pango::EllipsizeMode::End);
                                url.set_max_width_chars(32);
                                text.append(&title);
                                text.append(&url);
                                btn.set_child(Some(&text));
                                row.append(&btn);
                                let weak = Rc::downgrade(&b);
                                let uri = page.url.clone();
                                btn.connect_clicked(move |_| {
                                    if let Some(b) = weak.upgrade() {
                                        if let Some(id) = id {
                                            b.select(id);
                                        } else {
                                            b.navigate(&uri);
                                        }
                                        b.panel.set_visible(false);
                                    }
                                });
                                if kind == "Bookmarks" {
                                    let remove = icon("edit-delete-symbolic", "Remove bookmark");
                                    row.append(&remove);
                                    let weak = Rc::downgrade(&b);
                                    let uri = page.url;
                                    remove.connect_clicked(move |_| {
                                        if let Some(b) = weak.upgrade() {
                                            b.state.borrow_mut().bookmarks.retain(|p| p.url != uri);
                                            b.dirty.set(true);
                                            b.show_panel("Bookmarks");
                                            b.update_chrome();
                                        }
                                    });
                                }
                                list.append(&row);
                            }
                            if count == 0 {
                                list.append(&label("Nothing here yet.", "muted"));
                            }
                        }
                    }
                };
                render("");
                search.connect_search_changed(move |e| render(&e.text()));
                search.grab_focus();
                if kind == "History" {
                    let clear = gtk::Button::with_label("Clear browsing history…");
                    let weak = Rc::downgrade(self);
                    clear.connect_clicked(move |_| {
                        if let Some(b) = weak.upgrade() {
                            b.confirm_clear("history");
                        }
                    });
                    content.append(&clear);
                }
                if kind == "Bookmarks" {
                    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                    for (name, export) in [("Import JSON", false), ("Export JSON", true)] {
                        let btn = gtk::Button::with_label(name);
                        let weak = Rc::downgrade(self);
                        btn.connect_clicked(move |_| {
                            if let Some(b) = weak.upgrade() {
                                b.bookmark_file(export);
                            }
                        });
                        row.append(&btn);
                    }
                    content.append(&row);
                }
            }
            "Downloads" => {
                if self.downloads.borrow().is_empty() {
                    content.append(&label("Your downloads appear here.", "muted"));
                }
                for d in self.downloads.borrow().iter() {
                    let row = gtk::Box::new(gtk::Orientation::Vertical, 6);
                    let title = label(&d.title.borrow(), "");
                    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
                    title.set_max_width_chars(30);
                    title.set_xalign(0.0);
                    row.append(&title);
                    row.append(&label(&d.status.borrow(), "muted"));
                    let btn = gtk::Button::with_label(if d.done.get() {
                        "Show in folder"
                    } else {
                        "Cancel"
                    });
                    btn.set_sensitive(!d.failed.get());
                    let d = d.clone();
                    let parent = self.window.clone();
                    btn.connect_clicked(move |_| {
                        if !d.done.get() {
                            d.download.cancel();
                        } else if let Some(path) = d.download.destination() {
                            let path = std::path::Path::new(path.as_str());
                            if let Some(dir) = path.parent() {
                                gtk::FileLauncher::new(Some(&gio::File::for_path(dir))).launch(
                                    Some(&parent),
                                    gio::Cancellable::NONE,
                                    |_| {},
                                );
                            }
                        }
                    });
                    row.append(&btn);
                    content.append(&row);
                }
            }
            "Settings" => {
                content.append(&label("Search engine", ""));
                let options = ["DuckDuckGo", "Google", "Brave"];
                let drop = gtk::DropDown::from_strings(&options);
                drop.set_selected(
                    options
                        .iter()
                        .position(|s| *s == self.state.borrow().settings.search)
                        .unwrap_or(0) as u32,
                );
                let weak = Rc::downgrade(self);
                drop.connect_selected_notify(move |d| {
                    if let Some(b) = weak.upgrade() {
                        b.state.borrow_mut().settings.search =
                            options[d.selected() as usize].into();
                        b.dirty.set(true);
                    }
                });
                content.append(&drop);
                content.append(&label("Page appearance", ""));
                let modes = ["Theme", "Dark", "Light"];
                let drop = gtk::DropDown::from_strings(&modes);
                drop.set_selected(
                    modes
                        .iter()
                        .position(|s| *s == self.state.borrow().settings.dark)
                        .unwrap_or(0) as u32,
                );
                let weak = Rc::downgrade(self);
                drop.connect_selected_notify(move |d| {
                    if let Some(b) = weak.upgrade() {
                        b.state.borrow_mut().settings.dark = modes[d.selected() as usize].into();
                        b.apply_appearance();
                        b.dirty.set(true);
                    }
                });
                content.append(&drop);
                for (text, key, active) in [
                    (
                        "Restore tabs at startup",
                        "restore",
                        self.state.borrow().settings.restore,
                    ),
                    (
                        "Block common trackers and ads",
                        "block",
                        self.state.borrow().settings.block,
                    ),
                ] {
                    let check = gtk::CheckButton::with_label(text);
                    check.set_active(active);
                    let weak = Rc::downgrade(self);
                    check.connect_toggled(move |c| {
                        if let Some(b) = weak.upgrade() {
                            if key == "restore" {
                                b.state.borrow_mut().settings.restore = c.is_active();
                            } else {
                                b.state.borrow_mut().settings.block = c.is_active();
                                b.compile_filter();
                            }
                            b.dirty.set(true);
                        }
                    });
                    content.append(&check);
                }
                let note=label("Protection uses a small built-in list. It is not a replacement for a full filter-list extension. Changes apply to subsequent requests.","muted");
                note.set_wrap(true);
                content.append(&note);
                content.append(&label("Default page zoom", ""));
                let zoom = gtk::SpinButton::with_range(50.0, 200.0, 10.0);
                zoom.set_value(self.state.borrow().settings.zoom * 100.0);
                let weak = Rc::downgrade(self);
                zoom.connect_value_changed(move |s| {
                    if let Some(b) = weak.upgrade() {
                        b.state.borrow_mut().settings.zoom = s.value() / 100.0;
                        b.dirty.set(true);
                        if let Some(v) = b.view() {
                            v.set_zoom_level(s.value() / 100.0);
                        }
                    }
                });
                content.append(&zoom);
                let clear = gtk::Button::with_label("Clear cookies and website data…");
                let weak = Rc::downgrade(self);
                clear.connect_clicked(move |_| {
                    if let Some(b) = weak.upgrade() {
                        b.confirm_clear("sites");
                    }
                });
                content.append(&clear);
                let note=label("No account, sync or telemetry. Website permissions are requested as needed. Private tabs do not enter history or the saved session.","muted");
                note.set_wrap(true);
                content.append(&note);
            }
            "Site" => {
                if let Some(t) = self.tab() {
                    let page = t.page.borrow();
                    let site = origin(&page.url);
                    let text = label(&host(&page.url), "");
                    text.set_wrap(true);
                    content.append(&text);
                    content.append(&label(
                        if page.url.starts_with("https://") && !t.failed.get() {
                            "HTTPS connection"
                        } else {
                            "Connection is not verified as secure"
                        },
                        "muted",
                    ));
                    if let Some(site) = site {
                        let enabled = !self.state.borrow().allowed.contains(&site);
                        let check =
                            gtk::CheckButton::with_label("Use content protection on this site");
                        check.set_active(enabled);
                        check.set_sensitive(!t.private);
                        let weak = Rc::downgrade(self);
                        let origin = site.clone();
                        check.connect_toggled(move |c| {
                            if let Some(b) = weak.upgrade() {
                                let mut s = b.state.borrow_mut();
                                s.allowed.retain(|u| u != &origin);
                                if !c.is_active() {
                                    s.allowed.push(origin.clone());
                                }
                                drop(s);
                                b.dirty.set(true);
                                b.compile_filter();
                                b.notice(
                                    "Site protection updated. Reload this page to apply it fully.",
                                );
                            }
                        });
                        content.append(&check);
                        let restore = gtk::Button::with_label("Restore hidden elements");
                        let weak = Rc::downgrade(self);
                        restore.connect_clicked(move |_| {
                            if let Some(b) = weak.upgrade() {
                                b.state.borrow_mut().hidden.remove(&site);
                                b.dirty.set(true);
                                if let Some(t) = b.tab() {
                                    b.refresh_content(&t);
                                }
                                if let Some(v) = b.view() {
                                    v.reload();
                                }
                                b.notice("Hidden elements restored for this site.");
                            }
                        });
                        content.append(&restore);
                    }
                }
            }
            "About" => {
                content.append(&label("凪  Nagi", "brand"));
                content.append(&label(&format!("Version {}", VERSION), "muted"));
                let note=label("A quiet native browser for Omarchy.\n\nBuilt with Rust, GTK4 and WebKitGTK. Inspired by Search from Office Commun.\n\nThis first release does not include browser extensions, a password vault, sync or guaranteed DRM playback.","");
                note.set_wrap(true);
                note.set_xalign(0.0);
                content.append(&note);
            }
            _ => {}
        }
        self.panel.append(&scroll);
    }
    fn confirm_clear(self: &Rc<Self>, what: &str) {
        let sites = what == "sites";
        let dialog = gtk::AlertDialog::builder()
            .message(if sites {
                "Clear cookies and website data?"
            } else {
                "Clear browsing history?"
            })
            .detail(if sites {
                "You will be signed out of websites. Bookmarks are kept."
            } else {
                "This removes the local history list. Bookmarks and open tabs are kept."
            })
            .buttons(["Cancel", "Clear"])
            .cancel_button(0)
            .default_button(0)
            .build();
        let weak = Rc::downgrade(self);
        dialog.choose(Some(&self.window), gio::Cancellable::NONE, move |result| {
            if result != Ok(1) {
                return;
            }
            if let Some(b) = weak.upgrade() {
                if sites {
                    if let Some(manager) = b.session.website_data_manager() {
                        let weak = Rc::downgrade(&b);
                        let (tx, rx) = std::sync::mpsc::channel();
                        manager.clear(
                            webkit::WebsiteDataTypes::all(),
                            gtk::glib::TimeSpan::from_seconds(0),
                            gio::Cancellable::NONE,
                            move |r| {
                                let _ = tx.send(r.is_ok());
                            },
                        );
                        gtk::glib::timeout_add_local(
                            std::time::Duration::from_millis(100),
                            move || {
                                let Some(b) = weak.upgrade() else {
                                    return gtk::glib::ControlFlow::Break;
                                };
                                if let Ok(ok) = rx.try_recv() {
                                    b.notice(if ok {
                                        "Website data cleared."
                                    } else {
                                        "Website data could not be cleared."
                                    });
                                    gtk::glib::ControlFlow::Break
                                } else {
                                    gtk::glib::ControlFlow::Continue
                                }
                            },
                        );
                    }
                } else {
                    b.state.borrow_mut().history.clear();
                    b.dirty.set(true);
                    b.show_panel("History");
                }
            }
        });
    }
    fn bookmark_file(self: &Rc<Self>, export: bool) {
        let dialog = gtk::FileDialog::builder()
            .title(if export {
                "Export bookmarks"
            } else {
                "Import Nagi bookmarks"
            })
            .initial_name("nagi-bookmarks.json")
            .build();
        let weak = Rc::downgrade(self);
        if export {
            let data = serde_json::to_vec_pretty(&self.state.borrow().bookmarks).unwrap();
            dialog.save(Some(&self.window), gio::Cancellable::NONE, move |result| {
                if let Ok(file) = result {
                    file.replace_contents_async(
                        data,
                        None,
                        false,
                        gio::FileCreateFlags::PRIVATE,
                        gio::Cancellable::NONE,
                        move |result| {
                            if let Some(b) = weak.upgrade() {
                                b.notice(if result.is_ok() {
                                    "Bookmarks exported."
                                } else {
                                    "Could not export bookmarks."
                                });
                            }
                        },
                    );
                }
            });
        } else {
            dialog.open(Some(&self.window), gio::Cancellable::NONE, move |result| {
                if let Ok(file) = result {
                    file.load_contents_async(gio::Cancellable::NONE, move |result| {
                        if let Some(b) = weak.upgrade() {
                            match result.ok().and_then(|(bytes, _)| {
                                if bytes.len() <= 5_000_000 {
                                    serde_json::from_slice::<Vec<Page>>(&bytes).ok()
                                } else {
                                    None
                                }
                            }) {
                                Some(pages) => {
                                    let mut state = b.state.borrow_mut();
                                    for p in pages.into_iter().take(10000) {
                                        if url::Url::parse(&p.url).ok().is_some_and(|u| {
                                            ["http", "https"].contains(&u.scheme())
                                        }) && !state.bookmarks.iter().any(|old| old.url == p.url)
                                        {
                                            state.bookmarks.push(p);
                                        }
                                    }
                                    drop(state);
                                    b.dirty.set(true);
                                    b.show_panel("Bookmarks");
                                }
                                None => b.notice(
                                    "This is not a valid Nagi bookmarks JSON file (maximum 5 MB).",
                                ),
                            }
                        }
                    });
                }
            });
        }
    }
}
