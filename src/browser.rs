use crate::{core::*, storage, theme};
use gtk::{gio, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use webkit::prelude::*;

pub struct Tab {
    pub id: u64,
    pub page: RefCell<Page>,
    pub private: bool,
    pub view: RefCell<Option<webkit::WebView>>,
    pub holder: gtk::Box,
    pub button: gtk::Box,
    pub label: gtk::Label,
    pub reader: Cell<bool>,
    pub failed: Cell<bool>,
    pub picking: Cell<bool>,
}
pub struct DownloadRow {
    pub download: webkit::Download,
    pub title: RefCell<String>,
    pub status: RefCell<String>,
    pub done: Cell<bool>,
    pub failed: Cell<bool>,
}
pub struct Browser {
    pub window: gtk::ApplicationWindow,
    pub state: RefCell<State>,
    pub writer: RefCell<Option<storage::Writer>>,
    pub tabs: RefCell<Vec<Rc<Tab>>>,
    pub active: Cell<u64>,
    next: Cell<u64>,
    pub closed: RefCell<Vec<Page>>,
    pub stack: gtk::Stack,
    pub strip: gtk::Box,
    pub chrome: gtk::Box,
    pub address: gtk::Entry,
    pub back: gtk::Button,
    pub forward: gtk::Button,
    pub reload: gtk::Button,
    pub shield: gtk::Button,
    pub star: gtk::Button,
    pub progress: gtk::ProgressBar,
    pub status: gtk::Label,
    pub panel: gtk::Box,
    pub panel_kind: RefCell<String>,
    pub find_bar: gtk::Box,
    pub find_entry: gtk::Entry,
    pub notification: gtk::Box,
    pub notification_label: gtk::Label,
    pub session: webkit::NetworkSession,
    pub filter: RefCell<Option<webkit::UserContentFilter>>,
    pub filter_ready: Cell<bool>,
    pub filter_generation: Cell<u64>,
    pub pending: RefCell<Vec<(glib::WeakRef<webkit::WebView>, String)>>,
    pub downloads: RefCell<Vec<Rc<DownloadRow>>>,
    pub palette: RefCell<theme::Palette>,
    css: gtk::CssProvider,
    pub dirty: Cell<bool>,
    pub closing: Cell<bool>,
    pub storage_error: Option<String>,
}
pub fn icon(name: &str, tip: &str) -> gtk::Button {
    gtk::Button::builder()
        .icon_name(name)
        .tooltip_text(tip)
        .build()
}
pub fn label(text: &str, class: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    if !class.is_empty() {
        l.add_css_class(class);
    }
    l
}
pub fn clear(b: &gtk::Box) {
    while let Some(c) = b.first_child() {
        b.remove(&c);
    }
}
impl Browser {
    pub fn new(app: &gtk::Application) -> Rc<Self> {
        let path = state_dir().join("state.json");
        let (state, error) = match storage::read(&path) {
            Ok(s) => (s, None),
            Err(e) => (State::default(), Some(e)),
        };
        let saved = state.tabs.clone();
        let selected = state.active;
        let restore = state.settings.restore;
        let writer = if error.is_none() {
            Some(storage::Writer::new(path))
        } else {
            None
        };
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title(APP_NAME)
            .default_width(1180)
            .default_height(800)
            .build();
        window.add_css_class("browser");
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        window.set_child(Some(&root));
        let strip_line = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        strip_line.add_css_class("tab-strip");
        strip_line.set_margin_start(8);
        strip_line.set_margin_end(8);
        let brand = label("NAGI", "eyebrow");
        brand.set_tooltip_text(Some("Nagi — a quiet browser"));
        brand.set_margin_end(8);
        strip_line.append(&brand);
        let strip = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .hexpand(true)
            .child(&strip)
            .build();
        strip_line.append(&scroller);
        let plus = icon("list-add-symbolic", "New tab · Ctrl+T");
        strip_line.append(&plus);
        let tabs_button = icon("view-list-symbolic", "Search tabs · Ctrl+K");
        strip_line.append(&tabs_button);
        let controls = gtk::WindowControls::new(gtk::PackType::End);
        strip_line.append(&controls);
        root.append(&strip_line);
        let chrome = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        chrome.add_css_class("chrome");
        chrome.set_margin_start(10);
        chrome.set_margin_end(10);
        chrome.set_margin_top(3);
        chrome.set_margin_bottom(8);
        let back = icon("go-previous-symbolic", "Back · Alt+Left");
        let forward = icon("go-next-symbolic", "Forward · Alt+Right");
        let reload = icon("view-refresh-symbolic", "Reload · Ctrl+R");
        chrome.append(&back);
        chrome.append(&forward);
        chrome.append(&reload);
        let address = gtk::Entry::builder()
            .placeholder_text("Search or enter an address")
            .hexpand(true)
            .build();
        address.set_icon_from_icon_name(
            gtk::EntryIconPosition::Primary,
            Some("system-search-symbolic"),
        );
        chrome.append(&address);
        let shield = icon("security-high-symbolic", "Site protection");
        let star = icon("non-starred-symbolic", "Bookmark this page · Ctrl+D");
        let menu = gtk::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .tooltip_text("Browser menu")
            .build();
        chrome.append(&shield);
        chrome.append(&star);
        chrome.append(&menu);
        root.append(&chrome);
        let progress = gtk::ProgressBar::new();
        root.append(&progress);
        let notification = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        notification.add_css_class("notice");
        let notification_label = gtk::Label::new(None);
        notification_label.set_wrap(true);
        notification_label.set_hexpand(true);
        notification_label.set_xalign(0.0);
        notification.append(&notification_label);
        let dismiss = icon("window-close-symbolic", "Dismiss message");
        notification.append(&dismiss);
        notification.set_visible(false);
        root.append(&notification);
        let find_bar = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        find_bar.set_margin_start(12);
        find_bar.set_margin_end(12);
        let find_entry = gtk::Entry::builder()
            .placeholder_text("Find on page")
            .hexpand(true)
            .build();
        find_bar.append(&find_entry);
        let prev = icon("go-up-symbolic", "Previous match");
        let next = icon("go-down-symbolic", "Next match");
        let end = icon("window-close-symbolic", "Close find");
        find_bar.append(&prev);
        find_bar.append(&next);
        find_bar.append(&end);
        find_bar.set_visible(false);
        root.append(&find_bar);
        let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let stack = gtk::Stack::new();
        stack.set_hexpand(true);
        stack.set_vexpand(true);
        body.append(&stack);
        let panel = gtk::Box::new(gtk::Orientation::Vertical, 12);
        panel.add_css_class("panel");
        panel.set_size_request(330, -1);
        panel.set_visible(false);
        body.append(&panel);
        root.append(&body);
        let status = label("", "status");
        status.set_xalign(0.0);
        status.set_ellipsize(gtk::pango::EllipsizeMode::End);
        root.append(&status);
        let session = webkit::NetworkSession::new(
            Some(&data_dir().join("web").to_string_lossy()),
            Some(&cache_dir().join("web").to_string_lossy()),
        );
        let palette = theme::load().unwrap_or_default();
        let css = gtk::CssProvider::new();
        css.load_from_data(&theme::css(&palette));
        gtk::style_context_add_provider_for_display(
            &gtk::gdk::Display::default().unwrap(),
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        let b = Rc::new(Self {
            window,
            state: RefCell::new(state),
            writer: RefCell::new(writer),
            tabs: RefCell::new(vec![]),
            active: Cell::new(0),
            next: Cell::new(1),
            closed: RefCell::new(vec![]),
            stack,
            strip,
            chrome,
            address,
            back,
            forward,
            reload,
            shield,
            star,
            progress,
            status,
            panel,
            panel_kind: RefCell::new(String::new()),
            find_bar,
            find_entry,
            notification,
            notification_label,
            session,
            filter: RefCell::new(None),
            filter_ready: Cell::new(false),
            filter_generation: Cell::new(0),
            pending: RefCell::new(vec![]),
            downloads: RefCell::new(vec![]),
            palette: RefCell::new(palette),
            css,
            dirty: Cell::new(false),
            closing: Cell::new(false),
            storage_error: error,
        });
        b.actions(app);
        b.menu(&menu);
        b.apply_appearance();
        b.setup_downloads(&b.session);
        b.compile_filter();
        let weak = Rc::downgrade(&b);
        plus.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.new_tab("about:blank", false, true);
            }
        });
        let weak = Rc::downgrade(&b);
        tabs_button.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.show_panel("Tabs");
            }
        });
        let weak = Rc::downgrade(&b);
        b.address.connect_activate(move |entry| {
            if let Some(b) = weak.upgrade() {
                let input = entry.text().to_string();
                b.navigate(&input);
            }
        });
        let weak = Rc::downgrade(&b);
        b.back.connect_clicked(move |_| {
            if let Some(v) = weak.upgrade().and_then(|b| b.view()) {
                v.go_back();
            }
        });
        let weak = Rc::downgrade(&b);
        b.forward.connect_clicked(move |_| {
            if let Some(v) = weak.upgrade().and_then(|b| b.view()) {
                v.go_forward();
            }
        });
        let weak = Rc::downgrade(&b);
        b.reload.connect_clicked(move |_| {
            if let Some(v) = weak.upgrade().and_then(|b| b.view()) {
                if v.is_loading() {
                    v.stop_loading();
                } else {
                    v.reload();
                }
            }
        });
        let weak = Rc::downgrade(&b);
        b.star.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.bookmark();
            }
        });
        let weak = Rc::downgrade(&b);
        b.shield.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.show_panel("Site");
            }
        });
        let weak = Rc::downgrade(&b);
        dismiss.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.notification.set_visible(false);
            }
        });
        let weak = Rc::downgrade(&b);
        b.find_entry.connect_changed(move |e| {
            if let Some(v) = weak.upgrade().and_then(|b| b.view()) {
                if let Some(f) = v.find_controller() {
                    f.search(
                        &e.text(),
                        (webkit::FindOptions::CASE_INSENSITIVE | webkit::FindOptions::WRAP_AROUND)
                            .bits(),
                        10000,
                    );
                }
            }
        });
        let weak = Rc::downgrade(&b);
        next.connect_clicked(move |_| {
            if let Some(v) = weak.upgrade().and_then(|b| b.view()) {
                if let Some(f) = v.find_controller() {
                    f.search_next();
                }
            }
        });
        let weak = Rc::downgrade(&b);
        prev.connect_clicked(move |_| {
            if let Some(v) = weak.upgrade().and_then(|b| b.view()) {
                if let Some(f) = v.find_controller() {
                    f.search_previous();
                }
            }
        });
        let weak = Rc::downgrade(&b);
        end.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.close_find();
            }
        });
        let weak = Rc::downgrade(&b);
        b.window.connect_close_request(move |_| {
            if let Some(b) = weak.upgrade() {
                b.closing.set(true);
                b.save();
                for d in b.downloads.borrow().iter() {
                    if !d.done.get() {
                        d.download.cancel();
                    }
                }
            }
            glib::Propagation::Proceed
        });
        let weak = Rc::downgrade(&b);
        glib::timeout_add_local(std::time::Duration::from_millis(750), move || {
            let Some(b) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            if b.closing.get() {
                return glib::ControlFlow::Break;
            }
            if b.dirty.replace(false) {
                b.save();
            }
            let error = b.writer.borrow().as_ref().and_then(|w| w.error());
            if let Some(e) = error {
                b.notice(&e);
            }
            if let Some(p) = theme::load() {
                if p != *b.palette.borrow() {
                    b.css.load_from_data(&theme::css(&p));
                    *b.palette.borrow_mut() = p;
                    b.apply_appearance();
                }
            }
            glib::ControlFlow::Continue
        });
        if restore {
            for page in saved {
                let tab = b.new_tab(&page.url, false, false);
                *tab.page.borrow_mut() = page;
            }
        }
        if b.tabs.borrow().is_empty() {
            b.new_tab("about:blank", false, false);
        }
        let id = {
            let tabs = b.tabs.borrow();
            tabs.get(selected).or_else(|| tabs.first()).map(|t| t.id)
        };
        if let Some(id) = id {
            b.select(id);
        }
        if let Some(error) = &b.storage_error {
            b.notice(&format!(
                "{error} This session will not overwrite saved data."
            ));
        }
        b
    }
    pub fn tab(&self) -> Option<Rc<Tab>> {
        self.tabs
            .borrow()
            .iter()
            .find(|t| t.id == self.active.get())
            .cloned()
    }
    pub fn view(&self) -> Option<webkit::WebView> {
        self.tab().and_then(|t| t.view.borrow().clone())
    }
    pub fn notice(&self, text: &str) {
        self.notification_label.set_text(text);
        self.notification.set_visible(true);
    }
    pub fn save(&self) {
        let tabs = self.tabs.borrow();
        let mut state = self.state.borrow_mut();
        state.tabs = tabs
            .iter()
            .filter(|t| !t.private)
            .map(|t| t.page.borrow().clone())
            .collect();
        state.active = tabs
            .iter()
            .filter(|t| !t.private)
            .position(|t| t.id == self.active.get())
            .unwrap_or(0);
        if let Some(w) = self.writer.borrow().as_ref() {
            w.save(state.clone());
        }
    }
    pub fn new_tab(self: &Rc<Self>, uri: &str, private: bool, select: bool) -> Rc<Tab> {
        let id = self.next.get();
        self.next.set(id + 1);
        let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let button = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        button.add_css_class("tab");
        if private {
            button.add_css_class("private");
        }
        let title = label("New tab", "");
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.set_max_width_chars(24);
        title.set_width_chars(16);
        title.set_xalign(0.0);
        let select_button = gtk::Button::new();
        select_button.set_child(Some(&title));
        button.append(&select_button);
        let close = icon("window-close-symbolic", "Close tab");
        button.append(&close);
        self.strip.append(&button);
        self.stack.add_named(&holder, Some(&id.to_string()));
        let tab = Rc::new(Tab {
            id,
            page: RefCell::new(Page {
                url: uri.into(),
                title: if uri == "about:blank" {
                    "New tab".into()
                } else {
                    host(uri)
                },
                pinned: false,
            }),
            private,
            view: RefCell::new(None),
            holder,
            button,
            label: title,
            reader: Cell::new(false),
            failed: Cell::new(false),
            picking: Cell::new(false),
        });
        self.tabs.borrow_mut().push(tab.clone());
        let weak = Rc::downgrade(self);
        select_button.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.select(id);
            }
        });
        let weak = Rc::downgrade(self);
        close.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.close_tab(id);
            }
        });
        let click = gtk::GestureClick::new();
        click.set_button(2);
        let weak = Rc::downgrade(self);
        click.connect_released(move |_, _, _, _| {
            if let Some(b) = weak.upgrade() {
                b.close_tab(id);
            }
        });
        tab.button.add_controller(click);
        if select {
            self.select(id);
        }
        self.dirty.set(true);
        tab
    }
    pub fn select(self: &Rc<Self>, id: u64) {
        self.active.set(id);
        self.close_find();
        let Some(tab) = self.tab() else { return };
        for t in self.tabs.borrow().iter() {
            if t.id == id {
                t.button.add_css_class("active");
            } else {
                t.button.remove_css_class("active");
            }
        }
        self.stack.set_visible_child_name(&id.to_string());
        if tab.view.borrow().is_none() {
            if tab.page.borrow().url == "about:blank" {
                self.welcome(&tab);
            } else {
                let uri = tab.page.borrow().url.clone();
                let v = self.build_view(&tab, None);
                self.load(&v, &uri);
            }
        }
        self.update_chrome();
        self.dirty.set(true);
        if tab.page.borrow().url == "about:blank" {
            self.address.grab_focus();
        } else if let Some(v) = tab.view.borrow().as_ref() {
            v.grab_focus();
        }
    }
    pub fn navigate(self: &Rc<Self>, input: &str) {
        let result = address(input, &self.state.borrow().settings.search);
        match result {
            Ok(uri) => {
                let Some(tab) = self.tab() else { return };
                tab.reader.set(false);
                tab.failed.set(false);
                tab.page.borrow_mut().url = uri.clone();
                if uri == "about:blank" {
                    if let Some(v) = tab.view.borrow_mut().take() {
                        v.stop_loading();
                        tab.holder.remove(&v);
                    }
                    self.welcome(&tab);
                    self.update_chrome();
                } else {
                    let existing = tab.view.borrow().clone();
                    let v = existing.unwrap_or_else(|| self.build_view(&tab, None));
                    self.refresh_content(&tab);
                    self.load(&v, &uri);
                    v.grab_focus();
                }
                self.dirty.set(true);
            }
            Err(e) => self.notice(&e),
        }
    }
    pub fn close_tab(self: &Rc<Self>, id: u64) {
        let index = self.tabs.borrow().iter().position(|t| t.id == id);
        let Some(i) = index else { return };
        let tab = self.tabs.borrow_mut().remove(i);
        if !tab.private {
            let mut closed = self.closed.borrow_mut();
            closed.push(tab.page.borrow().clone());
            if closed.len() > 30 {
                closed.remove(0);
            }
        }
        if let Some(v) = tab.view.borrow_mut().take() {
            v.stop_loading();
            tab.holder.remove(&v);
        }
        self.stack.remove(&tab.holder);
        self.strip.remove(&tab.button);
        if self.tabs.borrow().is_empty() {
            self.new_tab("about:blank", false, true);
        } else if self.active.get() == id {
            let next = self.tabs.borrow()[i.min(self.tabs.borrow().len() - 1)].id;
            self.select(next);
        }
        self.dirty.set(true);
    }
    pub fn update_chrome(&self) {
        let Some(tab) = self.tab() else { return };
        let page = tab.page.borrow();
        if !self.address.has_focus() {
            self.address.set_text(if page.url == "about:blank" {
                ""
            } else {
                &page.url
            });
        }
        self.window.set_title(Some(&format!(
            "{}{} — Nagi",
            if tab.private { "Private · " } else { "" },
            page.title
        )));
        self.address.set_icon_from_icon_name(
            gtk::EntryIconPosition::Primary,
            Some(if tab.private {
                "view-conceal-symbolic"
            } else if page.url.starts_with("https://") && !tab.failed.get() {
                "channel-secure-symbolic"
            } else {
                "dialog-information-symbolic"
            }),
        );
        self.star.set_icon_name(
            if self
                .state
                .borrow()
                .bookmarks
                .iter()
                .any(|p| p.url == page.url)
            {
                "starred-symbolic"
            } else {
                "non-starred-symbolic"
            },
        );
        if let Some(v) = tab.view.borrow().as_ref() {
            self.back.set_sensitive(v.can_go_back());
            self.forward.set_sensitive(v.can_go_forward());
            self.progress.set_fraction(if v.is_loading() {
                v.estimated_load_progress()
            } else {
                0.0
            });
            self.reload.set_icon_name(if v.is_loading() {
                "process-stop-symbolic"
            } else {
                "view-refresh-symbolic"
            });
        } else {
            self.back.set_sensitive(false);
            self.forward.set_sensitive(false);
            self.progress.set_fraction(0.0);
        }
        for t in self.tabs.borrow().iter() {
            let p = t.page.borrow();
            t.label.set_text(&format!(
                "{}{}{}",
                if t.private { "◌ " } else { "" },
                if p.pinned { "• " } else { "" },
                p.title
            ));
            t.button.set_tooltip_text(Some(&p.url));
        }
    }
    pub fn welcome(self: &Rc<Self>, tab: &Rc<Tab>) {
        clear(&tab.holder);
        let area = gtk::Box::new(gtk::Orientation::Vertical, 16);
        area.add_css_class("welcome");
        area.set_vexpand(true);
        area.set_halign(gtk::Align::Center);
        area.set_valign(gtk::Align::Center);
        area.append(&label(
            if tab.private {
                "PRIVATE BROWSING"
            } else {
                "A LITTLE QUIETER"
            },
            "eyebrow",
        ));
        area.append(&label("Room to explore.", "brand"));
        area.append(&label(
            if tab.private {
                "This tab keeps no history or session. Downloads remain on disk."
            } else {
                "Your next thought starts here."
            },
            "muted",
        ));
        let entry = gtk::Entry::builder()
            .placeholder_text("Search the web or enter an address")
            .width_chars(48)
            .build();
        area.append(&entry);
        let weak = Rc::downgrade(self);
        entry.connect_activate(move |e| {
            if let Some(b) = weak.upgrade() {
                b.navigate(&e.text());
            }
        });
        let links = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        for (text, url) in [
            ("Omarchy", "https://omarchy.org"),
            ("GitHub", "https://github.com"),
            ("Bookmarks", ""),
        ] {
            let btn = gtk::Button::with_label(text);
            let weak = Rc::downgrade(self);
            btn.connect_clicked(move |_| {
                if let Some(b) = weak.upgrade() {
                    if url.is_empty() {
                        b.show_panel("Bookmarks");
                    } else {
                        b.navigate(url);
                    }
                }
            });
            links.append(&btn);
        }
        area.append(&links);
        let hint = label(
            "Ctrl+L  address     Ctrl+T  new tab     Ctrl+K  find a tab",
            "muted",
        );
        hint.set_margin_top(24);
        area.append(&hint);
        tab.holder.append(&area);
    }
    pub fn close_find(&self) {
        self.find_bar.set_visible(false);
        if let Some(v) = self.view() {
            if let Some(f) = v.find_controller() {
                f.search_finish();
            }
        }
    }
    pub fn apply_appearance(&self) {
        let mode = self.state.borrow().settings.dark.clone();
        let dark = if mode == "Theme" {
            self.palette.borrow().dark
        } else {
            mode == "Dark"
        };
        if let Some(s) = gtk::Settings::default() {
            s.set_gtk_application_prefer_dark_theme(dark);
        }
    }
    pub fn actions(self: &Rc<Self>, app: &gtk::Application) {
        for (name, keys) in [
            ("new", vec!["<Control>t"]),
            ("private", vec!["<Control><Shift>n"]),
            ("close", vec!["<Control>w"]),
            ("reopen", vec!["<Control><Shift>t"]),
            ("address", vec!["<Control>l"]),
            ("tabs", vec!["<Control>k"]),
            ("history", vec!["<Control>h"]),
            ("bookmarks", vec!["<Control>b"]),
            ("downloads", vec!["<Control>j"]),
            ("bookmark", vec!["<Control>d"]),
            ("settings", vec!["<Control>comma"]),
            ("find", vec!["<Control>f"]),
            ("reload", vec!["<Control>r", "F5"]),
            ("back", vec!["<Alt>Left"]),
            ("forward", vec!["<Alt>Right"]),
            ("next", vec!["<Control>Tab"]),
            ("previous", vec!["<Control><Shift>Tab"]),
            ("reader", vec!["<Control><Shift>r"]),
            ("hide", vec!["<Control><Shift>h"]),
            ("fullscreen", vec!["F11"]),
            ("escape", vec!["Escape"]),
            ("zoom-in", vec!["<Control>plus", "<Control>equal"]),
            ("zoom-out", vec!["<Control>minus"]),
            ("zoom-reset", vec!["<Control>0"]),
            ("pin", vec![]),
            ("duplicate", vec![]),
            ("mute", vec![]),
            ("about", vec![]),
            ("quit", vec!["<Control>q"]),
        ] {
            let action = gio::SimpleAction::new(name, None);
            let weak = Rc::downgrade(self);
            action.connect_activate(move |_, _| {
                if let Some(b) = weak.upgrade() {
                    b.command(name);
                }
            });
            self.window.add_action(&action);
            app.set_accels_for_action(&format!("win.{name}"), &keys);
        }
    }
    pub fn command(self: &Rc<Self>, name: &str) {
        match name {
            "new" => {
                self.new_tab("about:blank", false, true);
            }
            "private" => {
                self.new_tab("about:blank", true, true);
            }
            "close" => self.close_tab(self.active.get()),
            "address" => {
                self.address.grab_focus();
                self.address.select_region(0, -1);
            }
            "reopen" => {
                let p = self.closed.borrow_mut().pop();
                if let Some(p) = p {
                    let t = self.new_tab(&p.url, false, true);
                    *t.page.borrow_mut() = p;
                    self.update_chrome();
                }
            }
            "tabs" => self.show_panel("Tabs"),
            "history" => self.show_panel("History"),
            "bookmarks" => self.show_panel("Bookmarks"),
            "downloads" => self.show_panel("Downloads"),
            "settings" => self.show_panel("Settings"),
            "about" => self.show_panel("About"),
            "bookmark" => self.bookmark(),
            "find" => {
                self.find_bar.set_visible(true);
                self.find_entry.grab_focus();
            }
            "reload" => {
                if let Some(v) = self.view() {
                    v.reload();
                }
            }
            "back" => {
                if let Some(v) = self.view() {
                    v.go_back();
                }
            }
            "forward" => {
                if let Some(v) = self.view() {
                    v.go_forward();
                }
            }
            "next" | "previous" => {
                let tabs = self.tabs.borrow();
                let len = tabs.len();
                let i = tabs
                    .iter()
                    .position(|t| t.id == self.active.get())
                    .unwrap_or(0);
                let id = tabs[(i + if name == "next" { 1 } else { len - 1 }) % len].id;
                drop(tabs);
                self.select(id);
            }
            "reader" => self.reader(),
            "hide" => self.hide_element(),
            "fullscreen" => {
                if self.window.is_fullscreen() {
                    self.window.unfullscreen();
                } else {
                    self.window.fullscreen();
                }
            }
            "escape" => {
                self.close_find();
                self.panel.set_visible(false);
                if self.window.is_fullscreen() {
                    self.window.unfullscreen();
                }
                if let Some(v) = self.view() {
                    v.stop_loading();
                    v.grab_focus();
                }
            }
            "zoom-in" | "zoom-out" | "zoom-reset" => {
                if let Some(v) = self.view() {
                    let z = match name {
                        "zoom-in" => v.zoom_level() + 0.1,
                        "zoom-out" => v.zoom_level() - 0.1,
                        _ => 1.0,
                    };
                    v.set_zoom_level(z.clamp(0.3, 3.0));
                }
            }
            "pin" => {
                if let Some(t) = self.tab() {
                    let pinned = t.page.borrow().pinned;
                    t.page.borrow_mut().pinned = !pinned;
                    self.tabs
                        .borrow_mut()
                        .sort_by_key(|tab| !tab.page.borrow().pinned);
                    let mut previous: Option<gtk::Box> = None;
                    for tab in self.tabs.borrow().iter() {
                        self.strip
                            .reorder_child_after(&tab.button, previous.as_ref());
                        previous = Some(tab.button.clone());
                    }
                    self.dirty.set(true);
                    self.update_chrome();
                }
            }
            "duplicate" => {
                if let Some(t) = self.tab() {
                    let uri = t.page.borrow().url.clone();
                    self.new_tab(&uri, t.private, true);
                }
            }
            "mute" => {
                if let Some(v) = self.view() {
                    v.set_is_muted(!v.is_muted());
                }
            }
            "quit" => self.window.close(),
            _ => {}
        }
    }
    fn menu(&self, button: &gtk::MenuButton) {
        let menu = gio::Menu::new();
        for items in [
            vec![
                ("New tab", "new"),
                ("New private tab", "private"),
                ("Reopen closed tab", "reopen"),
            ],
            vec![
                ("Find a tab", "tabs"),
                ("Bookmarks", "bookmarks"),
                ("History", "history"),
                ("Downloads", "downloads"),
            ],
            vec![
                ("Pin / unpin tab", "pin"),
                ("Duplicate tab", "duplicate"),
                ("Mute / unmute tab", "mute"),
                ("Reader mode", "reader"),
                ("Hide an element", "hide"),
                ("Find on page", "find"),
            ],
            vec![
                ("Settings", "settings"),
                ("About Nagi", "about"),
                ("Quit", "quit"),
            ],
        ] {
            let section = gio::Menu::new();
            for (text, action) in items {
                section.append(Some(text), Some(&format!("win.{action}")));
            }
            menu.append_section(None, &section);
        }
        button.set_menu_model(Some(&menu));
    }
}
