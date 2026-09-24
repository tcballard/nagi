mod browser;
mod config;
mod config_store;
mod core;
mod icons;
mod import;
mod panels;
mod storage;
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
    if args.iter().any(|a| a == "--version") {
        println!("Nagi {}", core::VERSION);
        return gtk::glib::ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "--help") {
        println!("Nagi — a quiet native browser for Omarchy\n\nUsage: nagi [--private] [--focus-address] [URL ...]\n       nagi --version\n       nagi config get [key] | nagi config set KEY VALUE\n\nCtrl+Alt+L / Ctrl+L floating address bar · Ctrl+T new tab · Ctrl+W close tab\nCtrl+K tabs · Ctrl+H history · Ctrl+B bookmarks · Ctrl+J downloads\nCtrl+Shift+N private tab · Ctrl+Shift+T reopen tab\nCtrl+F find · Ctrl+D bookmark · Ctrl+Shift+R reader\nCtrl+, settings · F11 fullscreen");
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
            let b = browser::Browser::new(app);
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
        if focus_address {
            browser.show_search();
        }
        gtk::glib::ExitCode::SUCCESS
    });
    let code = app.run();
    if let Some(b) = holder.borrow_mut().take() {
        b.save();
        b.writer.borrow_mut().take();
    }
    code
}
