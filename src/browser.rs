use crate::{config, core::*, storage, theme};
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
    pub internal_icon: gtk::Image,
    pub favicon: gtk::Image,
    pub reader: Cell<bool>,
    pub failed: Cell<bool>,
    pub picking: Cell<bool>,
    pub generation: Cell<u64>,
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
    pub safe_mode: bool,
    pub control: RefCell<crate::control::Control>,
    pub extensions: RefCell<Vec<crate::extensions::Installed>>,
    pub extension_fingerprint:
        RefCell<Vec<(std::path::PathBuf, Option<std::time::SystemTime>, u64)>>,
    pub extension_views: RefCell<Vec<(String, glib::WeakRef<webkit::WebView>)>>,
    pub control_bar: gtk::Box,
    personal_css: gtk::CssProvider,
    personal_toolbar: gtk::Box,
    pub state: RefCell<State>,
    pub writer: RefCell<Option<storage::Writer>>,
    pub tabs: RefCell<Vec<Rc<Tab>>>,
    pub active: Cell<u64>,
    next: Cell<u64>,
    pub closed: RefCell<Vec<Page>>,
    pub stack: gtk::Stack,
    pub strip: gtk::Box,
    tab_sidebar: gtk::Box,
    top_scroller: gtk::ScrolledWindow,
    side_scroller: gtk::ScrolledWindow,
    app: gtk::Application,
    settings_snapshot: RefCell<Settings>,
    pub chrome: gtk::Box,
    root: gtk::Box,
    address_layer: gtk::Overlay,
    composer_revealer: gtk::Revealer,
    composer_open: Cell<bool>,
    suggestions: gtk::ListBox,
    suggestion_scroll: gtk::ScrolledWindow,
    matches: RefCell<Vec<crate::suggestions::Suggestion>>,
    address_button: gtk::Button,
    pub address: gtk::Entry,
    address_error: gtk::Label,
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
    pub fn new(app: &gtk::Application, safe_mode: bool) -> Rc<Self> {
        let path = state_dir().join("state.json");
        let (mut state, error) = match storage::read(&path) {
            Ok(s) => (s, None),
            Err(e) => (State::default(), Some(e)),
        };
        let config_error = match config::read() {
            Ok(Some(settings)) => {
                state.settings = settings;
                None
            }
            Ok(None) => None,
            Err(e) => Some(e),
        };
        if safe_mode {
            state.settings = Settings::default();
            state.settings.restore = false;
        }
        let saved = state.tabs.clone();
        let selected = state.active;
        let restore = state.settings.restore;
        let writer = if error.is_none() && !safe_mode {
            Some(storage::Writer::new(path))
        } else {
            None
        };
        let (width, height) = state.window.size();
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title(APP_NAME)
            .default_width(width)
            .default_height(height)
            .build();
        if state.window.maximized {
            window.maximize();
        }
        window.add_css_class("browser");
        window.set_icon_name(Some("nagi"));
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&root));
        window.set_child(Some(&overlay));
        let strip_line = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        strip_line.add_css_class("tab-strip");
        strip_line.set_margin_start(8);
        strip_line.set_margin_end(8);
        let brand = crate::icons::image(24);
        brand.set_valign(gtk::Align::Center);
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
        let side_scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .vexpand(true)
            .build();
        let tab_sidebar = gtk::Box::new(gtk::Orientation::Vertical, 4);
        tab_sidebar.add_css_class("tab-sidebar");
        tab_sidebar.set_size_request(210, -1);
        tab_sidebar.append(&side_scroller);
        tab_sidebar.set_visible(false);
        let plus = icon("list-add-symbolic", "New tab · Ctrl+T");
        strip_line.append(&plus);
        let tabs_button = icon("view-list-symbolic", "Search tabs · Ctrl+K");
        strip_line.append(&tabs_button);
        let address_button = icon(
            "system-search-symbolic",
            "Address / search · Ctrl+Alt+L or Ctrl+L",
        );
        strip_line.append(&address_button);
        let personal_toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        strip_line.append(&personal_toolbar);
        let menu = gtk::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .tooltip_text("Browser menu")
            .build();
        strip_line.append(&menu);
        let controls = gtk::WindowControls::new(gtk::PackType::End);
        strip_line.append(&controls);
        root.append(&strip_line);
        let control_bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        control_bar.add_css_class("notice");
        control_bar.append(&label(
            "Agent control enabled · access requires your approval",
            "",
        ));
        let stop_control = gtk::Button::with_label("Stop agent control");
        control_bar.append(&stop_control);
        control_bar.set_visible(false);
        root.append(&control_bar);
        let chrome = gtk::Box::new(gtk::Orientation::Vertical, 12);
        chrome.add_css_class("chrome");
        chrome.add_css_class("address-card");
        chrome.set_halign(gtk::Align::Center);
        chrome.set_valign(gtk::Align::Center);
        chrome.set_margin_start(24);
        chrome.set_margin_end(24);
        let address = gtk::Entry::builder()
            .placeholder_text("What are you looking for?")
            .width_chars(1)
            .max_width_chars(56)
            .hexpand(true)
            .build();
        address.add_css_class("composer-input");
        address.set_tooltip_text(Some("Search the web or enter a URL"));
        chrome.append(&address);
        let address_error = label("", "muted");
        address_error.set_wrap(true);
        address_error.set_visible(false);
        chrome.append(&address_error);
        let suggestions = gtk::ListBox::new();
        suggestions.set_selection_mode(gtk::SelectionMode::Single);
        suggestions.set_activate_on_single_click(true);
        suggestions.add_css_class("suggestions");
        let suggestion_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .max_content_height(264)
            .propagate_natural_height(true)
            .child(&suggestions)
            .visible(false)
            .build();
        chrome.append(&suggestion_scroll);
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let hint = label("Search or paste a link · Esc to close", "composer-hint");
        hint.set_hexpand(true);
        hint.set_xalign(0.0);
        hint.set_ellipsize(gtk::pango::EllipsizeMode::End);
        footer.append(&hint);
        let submit = icon("go-up-symbolic", "Go · Enter");
        submit.add_css_class("composer-submit");
        submit.set_sensitive(false);
        footer.append(&submit);
        chrome.append(&footer);
        let address_layer = gtk::Overlay::new();
        let backdrop = gtk::Button::new();
        backdrop.add_css_class("address-backdrop");
        backdrop.set_focusable(false);
        backdrop.set_hexpand(true);
        backdrop.set_vexpand(true);
        address_layer.set_child(Some(&backdrop));
        address_layer.add_overlay(&chrome);
        address_layer.set_can_target(false);
        let composer_revealer = gtk::Revealer::builder()
            .transition_type(gtk::RevealerTransitionType::Crossfade)
            .transition_duration(120)
            .child(&address_layer)
            .build();
        composer_revealer.set_can_target(false);
        overlay.add_overlay(&composer_revealer);
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
        body.append(&tab_sidebar);
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
        let personal_css = gtk::CssProvider::new();
        gtk::style_context_add_provider_for_display(
            &gtk::gdk::Display::default().unwrap(),
            &personal_css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
        );
        let settings_snapshot = state.settings.clone();
        let b = Rc::new(Self {
            window,
            safe_mode,
            control: RefCell::new(crate::control::Control::default()),
            extensions: RefCell::new(vec![]),
            extension_fingerprint: RefCell::new(vec![]),
            extension_views: RefCell::new(vec![]),
            control_bar,
            personal_css,
            personal_toolbar,
            state: RefCell::new(state),
            writer: RefCell::new(writer),
            tabs: RefCell::new(vec![]),
            active: Cell::new(0),
            next: Cell::new(1),
            closed: RefCell::new(vec![]),
            stack,
            strip,
            tab_sidebar,
            top_scroller: scroller,
            side_scroller,
            app: app.clone(),
            settings_snapshot: RefCell::new(settings_snapshot),
            chrome,
            root,
            address_layer,
            composer_revealer,
            composer_open: Cell::new(false),
            suggestions,
            suggestion_scroll,
            matches: RefCell::new(vec![]),
            address_button,
            address,
            address_error,
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
        let weak = Rc::downgrade(&b);
        stop_control.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.stop_control();
            }
        });
        let weak = Rc::downgrade(&b);
        b.suggestions.connect_row_activated(move |_, row| {
            if let Some(b) = weak.upgrade() {
                b.activate_suggestion(row.index() as usize);
            }
        });
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(&b);
        keys.connect_key_pressed(move |_, key, _, _| {
            if let Some(b) = weak.upgrade() {
                if key == gtk::gdk::Key::Down || key == gtk::gdk::Key::Up {
                    let count = b.matches.borrow().len() as i32;
                    if count > 0 {
                        let current = b
                            .suggestions
                            .selected_row()
                            .map(|r| r.index())
                            .unwrap_or(-1);
                        let next = if key == gtk::gdk::Key::Down {
                            (current + 1).min(count - 1)
                        } else {
                            current - 1
                        };
                        let row = b.suggestions.row_at_index(next);
                        b.suggestions.select_row(row.as_ref());
                        if let Some(bounds) = row.and_then(|r| r.compute_bounds(&b.suggestions)) {
                            let adjustment = b.suggestion_scroll.vadjustment();
                            let top = f64::from(bounds.y());
                            let bottom = top + f64::from(bounds.height());
                            if top < adjustment.value() {
                                adjustment.set_value(top);
                            } else if bottom > adjustment.value() + adjustment.page_size() {
                                adjustment.set_value(bottom - adjustment.page_size());
                            }
                        }
                        return glib::Propagation::Stop;
                    }
                }
            }
            glib::Propagation::Proceed
        });
        b.address.add_controller(keys);
        for property in ["default-width", "default-height", "maximized"] {
            let weak = Rc::downgrade(&b);
            b.window.connect_notify_local(Some(property), move |_, _| {
                if let Some(b) = weak.upgrade() {
                    b.dirty.set(true);
                }
            });
        }
        b.actions(app);
        b.apply_tab_layout();
        b.menu(&menu);
        b.apply_appearance();
        b.setup_downloads(&b.session);
        b.compile_filter();
        let weak = Rc::downgrade(&b);
        b.address_button.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.show_search();
            }
        });
        let weak = Rc::downgrade(&b);
        backdrop.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.dismiss_address();
            }
        });
        let weak = Rc::downgrade(&b);
        submit.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.submit_address();
            }
        });
        let weak = Rc::downgrade(&b);
        b.address.connect_changed(move |entry| {
            submit.set_sensitive(!entry.text().trim().is_empty());
            if let Some(b) = weak.upgrade() {
                b.refresh_suggestions();
            }
        });
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
        b.address.connect_activate(move |_| {
            if let Some(b) = weak.upgrade() {
                b.submit_address();
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
                b.stop_control();
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
            if let Ok(Some(settings)) = if b.safe_mode {
                Ok(None)
            } else {
                config::read()
            } {
                if settings != *b.settings_snapshot.borrow() {
                    let old = b.state.borrow().settings.clone();
                    b.state.borrow_mut().settings = settings.clone();
                    *b.settings_snapshot.borrow_mut() = settings.clone();
                    b.apply_personalisation();
                    b.apply_tab_layout();
                    b.apply_shortcuts();
                    b.apply_appearance();
                    if settings.block != old.block {
                        b.compile_filter();
                    }
                    if settings.zoom != old.zoom {
                        for t in b.tabs.borrow().iter() {
                            if let Some(v) = t.view.borrow().as_ref() {
                                v.set_zoom_level(settings.zoom);
                            }
                        }
                    }
                    if *b.panel_kind.borrow() == "Settings" && b.panel.is_visible() {
                        b.show_panel("Settings");
                    }
                    b.dirty.set(true);
                }
            }
            b.refresh_extensions();
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
        b.apply_personalisation();
        b.refresh_extensions();
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
        if safe_mode {
            b.notice("Safe mode: personalisation, extensions and agent control are disabled. Close Nagi before restarting normally.");
        }
        if let Some(e) = config_error {
            b.notice(&e);
        }
        if let Some(error) = &b.storage_error {
            b.notice(&format!(
                "{error} This session will not overwrite saved data."
            ));
        }
        b
    }
    pub fn set_preference(self: &Rc<Self>, key: &str, value: &str) -> Result<(), String> {
        if self.safe_mode {
            return Err("Settings are read-only in safe mode".into());
        }
        let next = config::change(key, value)?;
        self.apply_settings(next);
        Ok(())
    }
    pub fn undo_last_preference(self: &Rc<Self>) -> Result<(), String> {
        if self.safe_mode {
            return Err("Settings are read-only in safe mode".into());
        }
        let current = config::document()?;
        let previous = current
            .history
            .last()
            .ok_or("No settings change to undo")?
            .settings
            .clone();
        let restored = crate::config_store::transact(
            &config::path(),
            &current.settings,
            Some(current.revision),
            false,
            |d| {
                d.settings = previous;
                Ok(())
            },
        )?;
        self.apply_settings(restored.settings);
        Ok(())
    }
    fn apply_settings(self: &Rc<Self>, next: Settings) {
        let previous = self.state.borrow().settings.clone();
        self.state.borrow_mut().settings = next.clone();
        *self.settings_snapshot.borrow_mut() = next.clone();
        self.apply_personalisation();
        self.apply_tab_layout();
        self.apply_shortcuts();
        self.apply_appearance();
        if next.block != previous.block {
            self.compile_filter();
        }
        if next.zoom != previous.zoom {
            for t in self.tabs.borrow().iter() {
                if let Some(v) = t.view.borrow().as_ref() {
                    v.set_zoom_level(next.zoom);
                }
            }
        }
        self.dirty.set(true);
    }
    pub fn apply_personalisation(self: &Rc<Self>) {
        let settings = self.state.borrow().settings.clone();
        self.tab_sidebar
            .set_size_request(settings.sidebar_width, -1);
        let mut css = String::new();
        if settings.density == "Compact" {
            css.push_str(".browser .tab button { padding: 2px 4px; min-height: 22px; } .browser .tab-strip { padding: 2px; }");
        }
        if !settings.accent.is_empty() {
            css.push_str(&format!(".browser .tab.active {{ border-color: {}; }} .browser progressbar progress {{ background: {}; }}", settings.accent, settings.accent));
        }
        self.personal_css.load_from_data(&css);
        clear(&self.personal_toolbar);
        for action in settings.toolbar_actions {
            let button = gtk::Button::with_label(&action);
            let weak = Rc::downgrade(self);
            button.connect_clicked(move |_| {
                if let Some(b) = weak.upgrade() {
                    b.command(&action);
                }
            });
            self.personal_toolbar.append(&button);
        }
    }
    pub fn apply_tab_layout(&self) {
        let vertical = self.state.borrow().settings.tab_layout == "Left";
        if vertical {
            if self.side_scroller.child().as_ref() != Some(self.strip.upcast_ref()) {
                self.top_scroller.set_child(None::<&gtk::Widget>);
                self.strip.set_orientation(gtk::Orientation::Vertical);
                self.side_scroller.set_child(Some(&self.strip));
            }
        } else if self.top_scroller.child().as_ref() != Some(self.strip.upcast_ref()) {
            self.side_scroller.set_child(None::<&gtk::Widget>);
            self.strip.set_orientation(gtk::Orientation::Horizontal);
            self.top_scroller.set_child(Some(&self.strip));
        }
        self.top_scroller.set_visible(!vertical);
        self.tab_sidebar.set_visible(vertical);
        if vertical {
            self.strip.add_css_class("vertical-tabs");
        } else {
            self.strip.remove_css_class("vertical-tabs");
        }
    }
    pub fn apply_shortcuts(&self) {
        for (action, fallback) in [
            ("search", "<Control><Alt>l"),
            ("address", "<Control>l"),
            ("new", "<Control>t"),
            ("tabs", "<Control>k"),
            ("history", "<Control>h"),
            ("bookmarks", "<Control>b"),
            ("downloads", "<Control>j"),
            ("find", "<Control>f"),
            ("reload", "<Control>r"),
        ] {
            let accel = self
                .state
                .borrow()
                .settings
                .shortcuts
                .get(action)
                .cloned()
                .unwrap_or_else(|| fallback.to_string());
            self.app
                .set_accels_for_action(&format!("win.{action}"), &[accel.as_str()]);
        }
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
        let (width, height) = self.window.default_size();
        state.window = WindowState {
            width,
            height,
            maximized: self.window.is_maximized(),
        };
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
        let custom_url = self.state.borrow().settings.new_tab_url.clone();
        let uri = if uri == "about:blank" && !private && !self.safe_mode && !custom_url.is_empty() {
            custom_url.as_str()
        } else {
            uri
        };
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
        let tab_content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let internal_icon = crate::icons::image(16);
        internal_icon.set_visible(uri == "about:blank");
        tab_content.append(&internal_icon);
        let favicon = gtk::Image::from_icon_name("text-html-symbolic");
        favicon.set_pixel_size(16);
        favicon.set_visible(uri != "about:blank");
        tab_content.append(&favicon);
        tab_content.append(&title);
        select_button.set_child(Some(&tab_content));
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
            internal_icon,
            favicon,
            reader: Cell::new(false),
            failed: Cell::new(false),
            picking: Cell::new(false),
            generation: Cell::new(0),
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
        let drag = gtk::DragSource::new();
        drag.set_actions(gtk::gdk::DragAction::MOVE);
        drag.set_content(Some(&gtk::gdk::ContentProvider::for_value(
            &format!("nagi-tab-{id}").to_value(),
        )));
        tab.button.add_controller(drag);
        let drop = gtk::DropTarget::new(String::static_type(), gtk::gdk::DragAction::MOVE);
        let weak = Rc::downgrade(self);
        drop.connect_drop(move |_, value, _, _| {
            let Some(b) = weak.upgrade() else {
                return false;
            };
            let Ok(source) = value.get::<String>() else {
                return false;
            };
            let Some(source) = source
                .strip_prefix("nagi-tab-")
                .and_then(|s| s.parse::<u64>().ok())
            else {
                return false;
            };
            b.move_tab(source, id)
        });
        tab.button.add_controller(drop);
        if select {
            self.select(id);
        }
        self.dirty.set(true);
        tab
    }
    pub fn move_tab(&self, source: u64, destination: u64) -> bool {
        let mut tabs = self.tabs.borrow_mut();
        let Some(from) = tabs.iter().position(|t| t.id == source) else {
            return false;
        };
        let Some(to) = tabs.iter().position(|t| t.id == destination) else {
            return false;
        };
        if from == to {
            return true;
        }
        let moved = tabs.remove(from);
        tabs.insert(to, moved.clone());
        let preceding = to.checked_sub(1).map(|n| tabs[n].button.clone());
        self.strip
            .reorder_child_after(&moved.button, preceding.as_ref());
        self.dirty.set(true);
        true
    }
    pub fn select(self: &Rc<Self>, id: u64) {
        self.dismiss_address();
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
            self.show_search();
        } else if let Some(v) = tab.view.borrow().as_ref() {
            v.grab_focus();
        }
    }
    pub fn navigate(self: &Rc<Self>, input: &str) {
        let result = address(input, &self.state.borrow().settings.search);
        match result {
            Ok(uri) => {
                let Some(tab) = self.tab() else { return };
                self.dismiss_address();
                tab.reader.set(false);
                tab.failed.set(false);
                tab.page.borrow_mut().url = uri.clone();
                if uri == "about:blank" {
                    let removed = tab.view.borrow_mut().take();
                    if let Some(v) = removed {
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
            Err(e) => {
                if self.composer_open.get() {
                    self.address_error.set_text(&e);
                    self.address_error.set_visible(true);
                } else {
                    self.notice(&e);
                }
            }
        }
    }
    pub fn close_tab(self: &Rc<Self>, id: u64) {
        self.control_event("tab.closed", id);
        self.control.borrow_mut().shared.remove(&id);
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
        let removed = tab.view.borrow_mut().take();
        if let Some(v) = removed {
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
        self.address_button.set_tooltip_text(Some(&format!(
            "{}\nAddress / search · Ctrl+Alt+L or Ctrl+L",
            page.url
        )));
        if !self.composer_open.get() {
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
        if let Some(v) = tab.view.borrow().as_ref() {
            self.progress.set_fraction(if v.is_loading() {
                v.estimated_load_progress()
            } else {
                0.0
            });
        } else {
            self.progress.set_fraction(0.0);
        }
        for t in self.tabs.borrow().iter() {
            let p = t.page.borrow();
            t.internal_icon
                .set_visible(p.url == "about:blank" || t.reader.get());
            t.favicon
                .set_visible(p.url != "about:blank" && !t.reader.get());
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
        if !self.safe_mode && !tab.private {
            let id = self.state.borrow().settings.new_tab_extension.clone();
            if let Ok(extension) = crate::extensions::load(&id) {
                if extension.enabled && !extension.manifest.new_tab_html.is_empty() {
                    clear(&tab.holder);
                    let view = self.extension_view(&extension, &extension.manifest.new_tab_html);
                    tab.holder.append(&view);
                    return;
                }
            }
        }
        clear(&tab.holder);
        let area = gtk::Box::new(gtk::Orientation::Vertical, 16);
        area.add_css_class("welcome");
        area.append(&crate::icons::image(64));
        area.set_vexpand(true);
        area.set_halign(gtk::Align::Center);
        area.set_valign(gtk::Align::Center);
        let start = gtk::Button::with_label("Search or paste a link");
        let weak = Rc::downgrade(self);
        start.connect_clicked(move |_| {
            if let Some(b) = weak.upgrade() {
                b.show_search();
            }
        });
        area.append(&start);
        area.append(&label("Ctrl+Alt+L", "muted"));
        if tab.private {
            let privacy = label(
                "Private tab · History and session are not saved.\nDownloads remain on disk.",
                "muted",
            );
            privacy.set_wrap(true);
            privacy.set_justify(gtk::Justification::Center);
            area.append(&privacy);
        }
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
    fn refresh_suggestions(&self) {
        if !self.composer_open.get() {
            return;
        }
        while let Some(row) = self.suggestions.first_child() {
            self.suggestions.remove(&row);
        }
        let tabs: Vec<_> = self
            .tabs
            .borrow()
            .iter()
            .map(|t| (t.id, t.page.borrow().clone(), t.private))
            .collect();
        let private = self.tab().is_some_and(|t| t.private);
        let matches =
            crate::suggestions::find(&self.address.text(), &self.state.borrow(), &tabs, private);
        for item in &matches {
            let row = gtk::Box::new(gtk::Orientation::Vertical, 3);
            for (text, class) in [
                (&item.title[..], ""),
                (&format!("{} · {}", item.caption(), item.url), "muted"),
            ] {
                let line = label(text, class);
                line.set_xalign(0.0);
                line.set_ellipsize(gtk::pango::EllipsizeMode::End);
                line.set_max_width_chars(1);
                line.set_hexpand(true);
                row.append(&line);
            }
            self.suggestions.append(&row);
        }
        self.suggestions.unselect_all();
        self.suggestion_scroll.set_visible(!matches.is_empty());
        *self.matches.borrow_mut() = matches;
    }
    fn activate_suggestion(self: &Rc<Self>, index: usize) {
        let item = self.matches.borrow().get(index).cloned();
        if let Some(item) = item {
            match item.kind {
                crate::suggestions::Kind::Tab(id) => self.select(id),
                _ => self.navigate(&item.url),
            }
        }
    }
    fn submit_address(self: &Rc<Self>) {
        if let Some(row) = self.suggestions.selected_row() {
            self.activate_suggestion(row.index() as usize);
        } else {
            self.navigate(&self.address.text());
        }
    }
    pub fn show_search(&self) {
        if !self.composer_open.get() {
            self.address.set_text("");
        }
        self.focus_composer();
    }
    pub fn show_address(&self) {
        if !self.composer_open.get() {
            if let Some(tab) = self.tab() {
                let page = tab.page.borrow();
                self.address.set_text(if page.url == "about:blank" {
                    ""
                } else {
                    &page.url
                });
            }
        }
        self.focus_composer();
    }
    fn focus_composer(&self) {
        self.address_error.set_visible(false);
        self.chrome.set_visible(true);
        self.root.set_sensitive(false);
        self.composer_open.set(true);
        self.address_layer.set_can_target(true);
        self.composer_revealer.set_can_target(true);
        self.composer_revealer.set_reveal_child(true);
        self.refresh_suggestions();
        self.address.grab_focus();
        self.address.select_region(0, -1);
    }
    pub fn dismiss_address(&self) {
        if !self.composer_open.get() {
            return;
        }
        self.composer_open.set(false);
        self.address_layer.set_can_target(false);
        self.composer_revealer.set_can_target(false);
        self.composer_revealer.set_reveal_child(false);
        self.root.set_sensitive(true);
        if let Some(v) = self.view() {
            v.grab_focus();
        } else {
            self.address_button.grab_focus();
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
            ("search", vec!["<Control><Alt>l"]),
            ("site", vec![]),
            ("stop", vec![]),
            ("tabs", vec!["<Control>k"]),
            ("history", vec!["<Control>h"]),
            ("bookmarks", vec!["<Control>b"]),
            ("downloads", vec!["<Control>j"]),
            ("bookmark", vec!["<Control>d"]),
            ("settings", vec!["<Control>comma"]),
            ("extensions", vec![]),
            ("find", vec!["<Control>f"]),
            ("reload", vec!["<Control>r", "F5"]),
            ("back", vec!["<Alt>Left"]),
            ("forward", vec!["<Alt>Right"]),
            ("next", vec!["<Control>Tab"]),
            ("tab-1", vec!["<Control>1"]),
            ("tab-2", vec!["<Control>2"]),
            ("tab-3", vec!["<Control>3"]),
            ("tab-4", vec!["<Control>4"]),
            ("tab-5", vec!["<Control>5"]),
            ("tab-6", vec!["<Control>6"]),
            ("tab-7", vec!["<Control>7"]),
            ("tab-8", vec!["<Control>8"]),
            ("tab-9", vec!["<Control>9"]),
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
            let configured = self.state.borrow().settings.shortcuts.get(name).cloned();
            if let Some(accel) = configured {
                app.set_accels_for_action(&format!("win.{name}"), &[accel.as_str()]);
            } else {
                app.set_accels_for_action(&format!("win.{name}"), &keys);
            }
        }
    }
    pub fn command(self: &Rc<Self>, name: &str) {
        if name != "address" && name != "search" && name != "escape" {
            self.dismiss_address();
        }
        if let Some(digit) = name
            .strip_prefix("tab-")
            .and_then(|s| s.parse::<usize>().ok())
        {
            let tabs = self.tabs.borrow();
            let selected = if digit == 9 {
                tabs.last()
            } else {
                tabs.get(digit - 1)
            }
            .map(|t| t.id);
            drop(tabs);
            if let Some(id) = selected {
                self.select(id);
            }
            return;
        }
        match name {
            "new" => {
                self.new_tab("about:blank", false, true);
            }
            "private" => {
                self.new_tab("about:blank", true, true);
            }
            "close" => self.close_tab(self.active.get()),
            "address" => self.show_address(),
            "search" => self.show_search(),
            "site" => self.show_panel("Site"),
            "stop" => {
                if let Some(v) = self.view() {
                    v.stop_loading();
                }
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
            "extensions" => self.show_extensions(),
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
                if self.composer_open.get() {
                    self.dismiss_address();
                    return;
                }
                self.close_find();
                self.panel.set_visible(false);
                if self.window.is_fullscreen() {
                    self.window.unfullscreen();
                }
                self.chrome.set_visible(true);
                if let Some(tab) = self.tab() {
                    tab.picking.set(false);
                }
                if let Some(v) = self.view() {
                    v.evaluate_javascript("window.__nagiCancelPick?.(); if(document.fullscreenElement) document.exitFullscreen();", Some("nagi"), None, gio::Cancellable::NONE, |_| {});
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
                ("Search the web", "search"),
                ("Edit address", "address"),
                ("New tab", "new"),
                ("New private tab", "private"),
                ("Reopen closed tab", "reopen"),
            ],
            vec![
                ("Back", "back"),
                ("Forward", "forward"),
                ("Reload", "reload"),
                ("Stop loading", "stop"),
                ("Site information / protection", "site"),
                ("Bookmark this page", "bookmark"),
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
                ("Extensions", "extensions"),
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
