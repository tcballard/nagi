//! The UI and CLI share one small, atomic JSON settings document. The CLI can
//! change it while Nagi is running; the app reloads it on its existing timer.
use crate::core::{state_dir, Settings};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::PathBuf,
};

pub fn path() -> PathBuf { state_dir().join("settings.json") }

pub fn read() -> Result<Option<Settings>, String> {
    let path = path();
    if !path.exists() { return Ok(None); }
    let data = fs::read(&path).map_err(|e| format!("Could not read settings: {e}"))?;
    let settings: Settings = serde_json::from_slice(&data)
        .map_err(|e| format!("Invalid settings file (preserved): {e}"))?;
    validate(&settings)?;
    Ok(Some(settings))
}

pub fn write(settings: &Settings) -> Result<(), String> {
    validate(settings)?;
    let path = path();
    let dir = path.parent().ok_or("Missing settings directory")?;
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    let data = serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?;
    let mut f = OpenOptions::new().write(true).create(true).truncate(true).mode(0o600)
        .open(&tmp).map_err(|e| e.to_string())?;
    f.write_all(&data).and_then(|_| f.sync_all()).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    File::open(dir).and_then(|d| d.sync_all()).map_err(|e| e.to_string())?;
    Ok(())
}

pub const OVERRIDES: &[&str] = &["search", "address", "new", "tabs", "history", "bookmarks", "downloads", "find", "reload"];

pub fn validate(s: &Settings) -> Result<(), String> {
    if !["Top", "Left"].contains(&s.tab_layout.as_str()) { return Err("tabs.layout must be Top or Left".into()); }
    if !["DuckDuckGo", "Google", "Brave"].contains(&s.search.as_str()) { return Err("search.engine must be DuckDuckGo, Google or Brave".into()); }
    if !["Theme", "Dark", "Light"].contains(&s.dark.as_str()) { return Err("appearance must be Theme, Dark or Light".into()); }
    if !s.zoom.is_finite() || !(0.5..=2.0).contains(&s.zoom) { return Err("zoom must be 0.5 through 2.0".into()); }
    let mut seen = std::collections::HashSet::new();
    for (name, accel) in &s.shortcuts {
        if !OVERRIDES.contains(&name.as_str()) { return Err(format!("Unknown shortcut: {name}")); }
        let Some((key, mods)) = gtk::accelerator_parse(accel) else { return Err(format!("Invalid GTK shortcut: {accel}")); };
        if !mods.intersects(gtk::gdk::ModifierType::CONTROL_MASK | gtk::gdk::ModifierType::ALT_MASK)
            || key == gtk::gdk::Key::Tab || key == gtk::gdk::Key::Escape
        { return Err(format!("Shortcut must use Ctrl or Alt and must leave Tab/Escape to the page: {accel}")); }
        let canonical = gtk::accelerator_name(key, mods).to_string();
        if !seen.insert(canonical) { return Err("Two actions use the same shortcut".into()); }
    }
    Ok(())
}

pub fn set(s: &mut Settings, key: &str, value: &str) -> Result<(), String> {
    let mut next = s.clone();
    match key {
        "tabs.layout" => next.tab_layout = value.into(),
        "features.link_previews" => next.link_previews = parse_bool(value)?,
        "search.engine" => next.search = value.into(),
        "appearance" => next.dark = value.into(),
        "restore_tabs" => next.restore = parse_bool(value)?,
        "block_trackers" => next.block = parse_bool(value)?,
        "zoom" => next.zoom = value.parse().map_err(|_| "zoom must be a number")?,
        _ if key.starts_with("shortcuts.") => {
            let name = &key[10..];
            if !OVERRIDES.contains(&name) { return Err(format!("Unknown shortcut: {name}")); }
            if value == "default" { next.shortcuts.remove(name); }
            else { next.shortcuts.insert(name.to_owned(), value.to_owned()); }
        }
        _ => return Err(format!("Unknown setting: {key}")),
    }
    validate(&next)?;
    *s = next;
    Ok(())
}
fn parse_bool(s: &str) -> Result<bool, String> {
    match s { "true" => Ok(true), "false" => Ok(false), _ => Err("Use true or false".into()) }
}

pub fn cli(args: &[String]) -> i32 {
    let mut settings = match read() {
        Ok(Some(s)) => s,
        Ok(None) => match crate::storage::read(&state_dir().join("state.json")) {
            Ok(s) => s.settings,
            Err(e) => { eprintln!("{e}"); return 1; }
        },
        Err(e) => { eprintln!("{e}"); return 1; }
    };
    match args {
        [cmd] if cmd == "get" => {
            println!("{}", serde_json::to_string_pretty(&settings).unwrap()); 0
        }
        [cmd, key] if cmd == "get" => {
            let json = serde_json::to_value(&settings).unwrap();
            let found = match key.as_str() {
                "tabs.layout" => Some(json["tab_layout"].clone()),
                "features.link_previews" => Some(json["link_previews"].clone()),
                "search.engine" => Some(json["search"].clone()),
                "appearance" => Some(json["dark"].clone()),
                "restore_tabs" => Some(json["restore"].clone()),
                "block_trackers" => Some(json["block"].clone()),
                "zoom" => Some(json["zoom"].clone()),
                _ if key.starts_with("shortcuts.") && OVERRIDES.contains(&&key[10..]) => Some(json["shortcuts"][&key[10..]].clone()),
                _ => None,
            };
            if let Some(v) = found { println!("{v}"); 0 } else { eprintln!("Unknown setting: {key}"); 2 }
        }
        [cmd, key, value] if cmd == "set" => match set(&mut settings, key, value).and_then(|_| write(&settings)) {
            Ok(()) => { println!("{}", serde_json::to_string_pretty(&settings).unwrap()); 0 }
            Err(e) => { eprintln!("{e}"); 2 }
        },
        _ => { eprintln!("Usage: nagi config get [key] | nagi config set KEY VALUE"); 2 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_old_settings_and_rejects_invalid_mutations() {
        let mut s: Settings = serde_json::from_str(r#"{"search":"Google"}"#).unwrap();
        assert_eq!(s.tab_layout, "Top");
        set(&mut s, "tabs.layout", "Left").unwrap();
        set(&mut s, "shortcuts.search", "<Control><Alt>p").unwrap();
        assert!(set(&mut s, "shortcuts.search", "Tab").is_err());
        assert!(set(&mut s, "zoom", "NaN").is_err());
        assert!(set(&mut s, "features.link_previews", "yes").is_err());
        assert_eq!(s.shortcuts["search"], "<Control><Alt>p");
        assert_eq!(s.tab_layout, "Left");
    }
}
