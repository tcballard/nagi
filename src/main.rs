mod browser;
mod config;
mod config_store;
mod control;
mod control_transport;
mod core;
mod extension_host;
mod extensions;
mod icons;
mod import;
mod panels;
mod personal;
mod storage;
mod switch_log;
mod suggestions;
mod theme;
mod web;
use gtk::{gio, prelude::*};
use std::{cell::RefCell, rc::Rc};
fn main() -> gtk::glib::ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|a| a == "config") {
        return gtk::glib::ExitCode::from(config::cli(&args[2..]) as u8);
    }
    if args.get(1).is_some_and(|a| a == "extension") {
        return gtk::glib::ExitCode::from(extensions::cli(&args[2..]) as u8);
    }
    if args.get(1).is_some_and(|a| a == "browser") {
        return gtk::glib::ExitCode::from(control_transport::cli(&args[2..]) as u8);
    }
    if args.get(1).is_some_and(|a| a == "profile") {
        return gtk::glib::ExitCode::from(personal::cli(&args[2..]) as u8);
    }
    if args.get(1).is_some_and(|a| a == "log-switch") {
        return gtk::glib::ExitCode::from(switch_log::cli(&args[2..]) as u8);
    }
    if args.iter().any(|a| a == "--version") {
        println!("Nagi {}", core::VERSION);
        return gtk::glib::ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "--help") {
        println!("Nagi — a quiet native browser for Omarchy\n\nUsage: nagi [--private] [--focus-address] [--agent-control] [--safe-mode] [--extensions] [URL ...]\n       nagi --version\n       nagi config schema | inspect | get [key] | set KEY VALUE | apply JSON | undo\n       nagi profile export NAME | check | apply\n       nagi browser METHOD [JSON_PARAMS] [REQUEST_ID]\n       nagi extension schema | list | check | install | disable ID\n       nagi log-switch [SITE_OR_TASK REASON] | --summary [--since 7d] [--markdown]\n\nCtrl+Alt+L / Ctrl+L floating address bar · Ctrl+T new tab · Ctrl+W close tab\nCtrl+K tabs · Ctrl+H history · Ctrl+B bookmarks · Ctrl+J downloads\nCtrl+Shift+N private tab · Ctrl+Shift+T reopen tab\nCtrl+F find · Ctrl+D bookmark · Ctrl+Shift+R reader\nCtrl+, settings · F11 fullscreen");
        return gtk::glib::ExitCode::SUCCESS;
    }
    let app = gtk::Application::new(
        Some(core::APP_ID),
        gio::ApplicationFlags::HANDLES_COMMAND_LINE,
    );
    app.add_main_option(
        "focus-address",
        0u8.into(),
        gtk::glib::OptionFlags::NONE,
        gtk::glib::OptionArg::None,
        "Present Nagi and summon the floating address bar",
        None,
    );
    app.add_main_option(
        "extensions",
        0u8.into(),
        gtk::glib::OptionFlags::NONE,
        gtk::glib::OptionArg::None,
        "Open installed extensions",
        None,
    );
    app.add_main_option(
        "agent-control",
        0u8.into(),
        gtk::glib::OptionFlags::NONE,
        gtk::glib::OptionArg::None,
        "Enable local agent control; approve origin grants in Nagi",
        None,
    );
    app.add_main_option(
        "safe-mode",
        0u8.into(),
        gtk::glib::OptionFlags::NONE,
        gtk::glib::OptionArg::None,
        "Start with default settings and extensions/control disabled; no session writes",
        None,
    );
    app.add_main_option(
        "private",
        0u8.into(),
        gtk::glib::OptionFlags::NONE,
        gtk::glib::OptionArg::None,
        "Open a private tab",
        None,
    );
    let holder: Rc<RefCell<Option<Rc<browser::Browser>>>> = Rc::new(RefCell::new(None));
    let slot = holder.clone();
    app.connect_command_line(move |app, cmd| {
        let args: Vec<String> = cmd
            .arguments()
            .iter()
            .skip(1)
            .map(|s| s.to_string_lossy().to_string())
            .collect();
        let private = cmd
            .options_dict()
            .lookup::<bool>("private")
            .ok()
            .flatten()
            .unwrap_or(false);
        let focus_address = cmd
            .options_dict()
            .lookup::<bool>("focus-address")
            .ok()
            .flatten()
            .unwrap_or(false);
        let existing = slot.borrow().clone();
        let browser = existing.unwrap_or_else(|| {
            let safe = cmd
                .options_dict()
                .lookup::<bool>("safe-mode")
                .ok()
                .flatten()
                .unwrap_or(false);
            let b = browser::Browser::new(app, safe);
            if cmd
                .options_dict()
                .lookup::<bool>("agent-control")
                .ok()
                .flatten()
                .unwrap_or(false)
            {
                b.start_control();
            }
            *slot.borrow_mut() = Some(b.clone());
            b
        });
        let urls: Vec<_> = args.iter().filter(|s| !s.starts_with("--")).collect();
        if urls.is_empty() {
            if private {
                browser.new_tab("about:blank", true, true);
            }
        } else {
            for url in urls {
                match core::address(url, &{ browser.state.borrow().settings.search.clone() }) {
                    Ok(uri) => {
                        browser.new_tab(&uri, private, true);
                    }
                    Err(e) => browser.notice(&e),
                }
            }
        }
        browser.window.present();
        if cmd
            .options_dict()
            .lookup::<bool>("extensions")
            .ok()
            .flatten()
            .unwrap_or(false)
        {
            browser.show_extensions();
        }
        if focus_address {
            browser.show_search();
        }
        gtk::glib::ExitCode::SUCCESS
    });
    let code = app.run();
    if let Some(b) = holder.borrow_mut().take() {
        b.stop_control();
        b.save();
        b.writer.borrow_mut().take();
    }
    code
}
