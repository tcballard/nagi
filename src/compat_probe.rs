//! A deliberately separate, one-shot WebKitGTK probe for the compatibility
//! suite. There is no agent-control socket or persistent API in this process.
use crate::{browser::Browser, core};
use gtk::{gio, glib, prelude::*};
use serde_json::{json, Value};
use std::{
    cell::Cell,
    fs,
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};
use webkit::prelude::*;

fn finish(app: &gtk::Application, output: &PathBuf, done: &Cell<bool>, result: Value) {
    if done.replace(true) {
        return;
    }
    if let Err(error) = fs::write(output, format!("{result}\n")) {
        eprintln!("Compatibility result could not be saved: {error}");
    }
    app.quit();
}

fn run(args: &[String]) -> Result<i32, String> {
    if args.len() != 4 {
        return Err("Usage: nagi compat-probe SITE_ID URL CHECKS_JSON RESULT_PATH".into());
    }
    let id = args[0].clone();
    let uri = args[1].clone();
    if core::origin(&uri).is_none() {
        return Err("Probe URL must be HTTP(S)".into());
    }
    let checks: Value = serde_json::from_str(&args[2]).map_err(|e| e.to_string())?;
    if !checks.is_array() {
        return Err("Checks must be a JSON array".into());
    }
    let output = PathBuf::from(&args[3]);
    if !output.parent().is_some_and(|p| p.is_dir()) {
        return Err("Result directory does not exist".into());
    }
    // The runner is never an agent browsing session and never accepts an
    // arbitrary script from the site inventory.
    std::env::set_var("NAGI_COMPAT_PROBE", "1");
    let app = gtk::Application::new(Some(core::APP_ID), gio::ApplicationFlags::NON_UNIQUE);
    let output_result_path = output.clone();
    app.connect_activate(move |app| {
        let started = Instant::now();
        let browser = Browser::new(app, true);
        let tab = browser.new_tab(&uri, false, true);
        browser.window.present();
        let Some(view) = tab.view.borrow().as_ref().cloned() else {
            eprintln!("Probe did not create a WebView");
            app.quit();
            return;
        };
        let done = Rc::new(Cell::new(false));
        let loaded = Rc::new(Cell::new(false));
        let app_timeout = app.clone();
        let output_timeout = output.clone();
        let done_timeout = done.clone();
        let id_timeout = id.clone();
        let loaded_timeout = loaded.clone();
        glib::timeout_add_local(Duration::from_secs(20), move || {
            if !loaded_timeout.get() {
                finish(&app_timeout, &output_timeout, &done_timeout,
                    json!({"id":id_timeout,"status":"fail","checks":{"loads":{"ok":false,"reason":"20 second timeout"}},"elapsed_ms":20000,"console_error_count":0,"screenshot":null}));
            }
            glib::ControlFlow::Break
        });
        let app_timeout = app.clone();
        let output_timeout = output.clone();
        let done_timeout = done.clone();
        let id_timeout = id.clone();
        glib::timeout_add_local(Duration::from_secs(43), move || {
            finish(&app_timeout, &output_timeout, &done_timeout,
                json!({"id":id_timeout,"status":"fail","checks":{"loads":{"ok":true},"probe":{"ok":false,"reason":"43 second overall timeout"}},"elapsed_ms":43000,"console_error_count":0,"screenshot":null}));
            glib::ControlFlow::Break
        });
        let app_loaded = app.clone();
        let output_loaded = output.clone();
        let done_loaded = done.clone();
        let id_loaded = id.clone();
        let checks_loaded = checks.clone();
        let tab_loaded = tab.clone();
        let loaded_event = loaded.clone();
        view.connect_load_changed(move |view, event| {
            if event != webkit::LoadEvent::Finished || done_loaded.get() {
                return;
            }
            loaded_event.set(true);
            let elapsed = started.elapsed().as_millis();
            if tab_loaded.failed.get() || view.uri().as_deref().is_some_and(|u| u.starts_with("webkit")) {
                finish(&app_loaded, &output_loaded, &done_loaded,
                    json!({"id":id_loaded,"status":"fail","checks":{"loads":{"ok":false,"reason":"WebKit load error"}},"elapsed_ms":elapsed,"console_error_count":0,"screenshot":null}));
                return;
            }
            let script = format!(r#"JSON.stringify((()=>{{
              const checks={checks_loaded}; const result={{}};
              if(checks.some(c=>c.type==='drm_probe')){{
                window.__nagiDrmProbe='pending';
                navigator.requestMediaKeySystemAccess('com.widevine.alpha',[
                  {{initDataTypes:['cenc'],videoCapabilities:[{{contentType:'video/mp4; codecs="avc1.42E01E"'}}]}}
                ]).then(()=>window.__nagiDrmProbe='supported', e=>window.__nagiDrmProbe='unsupported: '+e.name);
              }}
              for(const check of checks){{
                if(check.type==='auth_persisted') result.auth_persisted=!!(check.selector&&document.querySelector(check.selector));
                if(check.type==='interaction'){{
                  const input=document.querySelector(check.selector||'');
                  if(input){{input.value=check.text||'';input.dispatchEvent(new Event('input',{{bubbles:true}}));input.dispatchEvent(new Event('change',{{bubbles:true}}));}}
                  result.interaction=!!input&&!!document.querySelector(check.expect_selector||'');
                }}
                if(check.type==='media_playback'){{
                  const media=document.querySelector(check.selector||'video, audio');
                  result.media_start=media?media.currentTime:null;
                  if(media)media.play().catch(()=>{{}});
                }}
              }}
              result.error_count=(window.__nagiCompatErrors||[]).length;
              result.is_error_page=!!document.querySelector('body.error-page');
              return result;
            }})())"#);
            let app_dom = app_loaded.clone();
            let output_dom = output_loaded.clone();
            let done_dom = done_loaded.clone();
            let id_dom = id_loaded.clone();
            let checks_dom = checks_loaded.clone();
            let view_dom = view.clone();
            view.evaluate_javascript(&script, Some("nagi-compat"), None, gio::Cancellable::NONE, move |js| {
                if done_dom.get() { return; }
                let dom: Value = js.ok().and_then(|v| serde_json::from_str(v.to_str().as_str()).ok()).unwrap_or(json!({"probe_error":true}));
                let app_shot = app_dom.clone();
                let output_shot = output_dom.clone();
                let done_shot = done_dom.clone();
                let id_shot = id_dom.clone();
                let checks_shot = checks_dom.clone();
                let view_shot = view_dom.clone();
                // Allow first paint and a media progression interval. A failed
                // snapshot remains an honest failure, never a fabricated pass.
                let delay = if checks_dom
                    .as_array()
                    .is_some_and(|c| c.iter().any(|c| c["type"] == "media_playback"))
                {
                    15
                } else {
                    3
                };
                glib::timeout_add_local(Duration::from_secs(delay), move || {
                    if done_shot.get() { return glib::ControlFlow::Break; }
                    let screenshot = output_shot.with_extension("png");
                    let app_end = app_shot.clone();
                    let output_end = output_shot.clone();
                    let done_end = done_shot.clone();
                    let id_end = id_shot.clone();
                    let checks_end = checks_shot.clone();
                    let dom_end = dom.clone();
                    let view_end = view_shot.clone();
                    view_shot.snapshot(webkit::SnapshotRegion::FullDocument, webkit::SnapshotOptions::NONE,
                        gio::Cancellable::NONE, move |image| {
                        let saved = image.ok().and_then(|surface| {
                            let png = surface.save_to_png_bytes();
                            if png.len() > 20 * 1024 * 1024 { return None; }
                            fs::write(&screenshot, &png).ok().map(|_| screenshot.to_string_lossy().into_owned())
                        });
                        let script = "JSON.stringify({media_end:(document.querySelector('video, audio')||{}).currentTime||null,error_count:(window.__nagiCompatErrors||[]).length,drm_probe:window.__nagiDrmProbe||null})";
                        let app_result = app_end.clone();
                        let output_result = output_end.clone();
                        let done_result = done_end.clone();
                        view_end.evaluate_javascript(script, Some("nagi-compat"), None, gio::Cancellable::NONE, move |after| {
                            let later: Value = after.ok().and_then(|v| serde_json::from_str(v.to_str().as_str()).ok()).unwrap_or(json!({}));
                            let errors = later["error_count"].as_u64().unwrap_or(0);
                            let auth = checks_end.as_array().is_some_and(|c| c.iter().any(|c| c["type"]=="auth_persisted"));
                            let interaction = checks_end.as_array().is_some_and(|c| c.iter().any(|c| c["type"]=="interaction"));
                            let media = checks_end.as_array().is_some_and(|c| c.iter().any(|c| c["type"]=="media_playback"));
                            let media_ok = later["media_end"].as_f64().zip(dom_end["media_start"].as_f64()).is_some_and(|(end,start)| end-start>=2.0);
                            let status = if saved.is_none() || dom_end["probe_error"]==true || dom_end["is_error_page"]==true {"fail"}
                                else if auth && dom_end["auth_persisted"]!=true {"blocked-auth"}
                                else if errors>0 || (interaction && dom_end["interaction"]!=true) || (media && !media_ok) {"partial"}
                                else {"pass"};
                            let rendered = saved.is_some();
                            finish(&app_result,&output_result,&done_result,json!({
                                "id":id_end,"status":status,"elapsed_ms":started.elapsed().as_millis(),
                                "console_error_count":errors,"screenshot":saved,"drm_probe":later["drm_probe"],
                                "checks":{"loads":{"ok":true},"renders":{"ok":rendered},
                                    "auth_persisted":{"ok":dom_end["auth_persisted"]==true},
                                    "interaction":{"ok":dom_end["interaction"]==true},
                                    "media_playback":{"ok":media_ok},
                                    "no_blocker_console_errors":{"ok":errors==0}}
                            }));
                        });
                    });
                    glib::ControlFlow::Break
                });
            });
        });
    });
    // Application::run() would reinterpret the probe arguments as files to
    // open. This dedicated instance receives no GTK command-line arguments.
    let _ = app.run_with_args(&["nagi-compat"]);
    Ok(if output_result_path.exists() { 0 } else { 2 })
}

pub fn cli(args: &[String]) -> i32 {
    match run(args) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            2
        }
    }
}
