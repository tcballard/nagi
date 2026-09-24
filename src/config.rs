//! CLI and GTK use the same validated transaction path.
use crate::{
    config_store::{self, Document},
    core::{state_dir, Settings},
};
use std::path::PathBuf;

pub fn path() -> PathBuf {
    state_dir().join("settings.json")
}
pub fn read() -> Result<Option<Settings>, String> {
    Ok(config_store::read(&path())?.map(|d| d.settings))
}
pub fn document() -> Result<Document, String> {
    if let Some(d) = config_store::read(&path())? {
        return Ok(d);
    }
    Ok(Document::initial(
        crate::storage::read(&state_dir().join("state.json"))?.settings,
    ))
}
pub fn change(key: &str, value: &str) -> Result<Settings, String> {
    let initial = document()?;
    Ok(
        config_store::transact(&path(), &initial.settings, None, false, |d| {
            set(&mut d.settings, key, value)
        })?
        .settings,
    )
}
pub fn schema() -> serde_json::Value {
    serde_json::json!({"api":1,"schema":1,"settings":{
        "tabs.layout":{"type":"string","enum":["Top","Left"],"default":"Top"},
        "features.link_previews":{"type":"boolean","default":false},
        "search.engine":{"type":"string","enum":["DuckDuckGo","Google","Brave"],"default":"DuckDuckGo"},
        "appearance":{"type":"string","enum":["Theme","Dark","Light"],"default":"Theme"},
        "restore_tabs":{"type":"boolean","default":true},
        "block_trackers":{"type":"boolean","default":true},
        "zoom":{"type":"number","minimum":0.5,"maximum":2.0,"default":1.0},
        "layout.density":{"type":"string","enum":["Comfortable","Compact"],"default":"Comfortable"},
        "tabs.sidebar_width":{"type":"integer","minimum":140,"maximum":420,"default":210},
        "appearance.accent":{"type":"string","format":"empty or #RRGGBB","default":""},
        "new_tab.extension":{"type":"string","format":"installed extension ID or empty","default":""},
        "new_tab.url":{"type":"string","format":"empty or HTTP(S) URL","default":""},
        "toolbar.actions":{"type":"array","items":TOOLBAR_ACTIONS,"default":[]},
        "shortcuts":{"actions":OVERRIDES,"reset":"default"}
    }})
}

pub const OVERRIDES: &[&str] = &[
    "search",
    "address",
    "new",
    "tabs",
    "history",
    "bookmarks",
    "downloads",
    "find",
    "reload",
];

pub const TOOLBAR_ACTIONS: &[&str] = &[
    "back",
    "forward",
    "reload",
    "bookmarks",
    "history",
    "downloads",
    "reader",
    "settings",
];
pub fn validate(s: &Settings) -> Result<(), String> {
    if !s.new_tab_extension.is_empty() && !crate::extensions::valid_id(&s.new_tab_extension) {
        return Err("Invalid new-tab extension ID".into());
    }
    if !["Comfortable", "Compact"].contains(&s.density.as_str()) {
        return Err("layout.density must be Comfortable or Compact".into());
    }
    if !(140..=420).contains(&s.sidebar_width) {
        return Err("Sidebar width must be 140–420".into());
    }
    if !s.accent.is_empty()
        && !(s.accent.len() == 7
            && s.accent.starts_with('#')
            && s.accent[1..].bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err("Accent must be empty or #RRGGBB".into());
    }
    if !s.new_tab_url.is_empty() {
        crate::personal::http_url(&s.new_tab_url)?;
    }
    if s.toolbar_actions.len() > 8
        || s.toolbar_actions
            .iter()
            .any(|a| !TOOLBAR_ACTIONS.contains(&a.as_str()))
    {
        return Err("Invalid toolbar actions".into());
    }
    if !["Top", "Left"].contains(&s.tab_layout.as_str()) {
        return Err("tabs.layout must be Top or Left".into());
    }
    if !["DuckDuckGo", "Google", "Brave"].contains(&s.search.as_str()) {
        return Err("search.engine must be DuckDuckGo, Google or Brave".into());
    }
    if !["Theme", "Dark", "Light"].contains(&s.dark.as_str()) {
        return Err("appearance must be Theme, Dark or Light".into());
    }
    if !s.zoom.is_finite() || !(0.5..=2.0).contains(&s.zoom) {
        return Err("zoom must be 0.5 through 2.0".into());
    }
    for name in s.shortcuts.keys() {
        if !OVERRIDES.contains(&name.as_str()) {
            return Err(format!("Unknown shortcut: {name}"));
        }
    }
    let mut seen = std::collections::HashSet::new();
    for (name, fallback) in [
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
        let accel = s
            .shortcuts
            .get(name)
            .map(String::as_str)
            .unwrap_or(fallback);
        let canonical = canonical_shortcut(accel)?;
        if !seen.insert(canonical) {
            return Err(format!(
                "Shortcut for {name} conflicts with another browser action"
            ));
        }
    }
    for reserved in [
        "<Control>Tab",
        "<Control><Shift>Tab",
        "<Control>w",
        "<Control><Shift>n",
        "<Control>1",
        "<Control>2",
        "<Control>3",
        "<Control>4",
        "<Control>5",
        "<Control>6",
        "<Control>7",
        "<Control>8",
        "<Control>9",
    ] {
        let canonical = reserved.to_ascii_lowercase();
        if !seen.insert(canonical) {
            return Err(format!(
                "Shortcut {reserved} is reserved for another browser action"
            ));
        }
    }
    Ok(())
}

fn canonical_shortcut(accel: &str) -> Result<String, String> {
    let mut rest = accel;
    let mut control = false;
    let mut alt = false;
    let mut shift = false;
    while let Some(modifier) = rest.strip_prefix('<') {
        let Some((name, tail)) = modifier.split_once('>') else {
            break;
        };
        match name.to_ascii_lowercase().as_str() {
            "control" | "ctrl" if !control => control = true,
            "alt" if !alt => alt = true,
            "shift" if !shift => shift = true,
            _ => return Err(format!("Unsupported modifier: {name}")),
        }
        rest = tail;
    }
    let key = rest.to_ascii_lowercase();
    let valid = (key.len() == 1 && key.bytes().all(|c| c.is_ascii_alphanumeric()))
        || [
            "plus",
            "equal",
            "minus",
            "comma",
            "bracketleft",
            "bracketright",
        ]
        .contains(&key.as_str())
        || key
            .strip_prefix('f')
            .and_then(|n| n.parse::<u8>().ok())
            .is_some_and(|n| (1..=12).contains(&n));
    if !valid || !(control || alt) || (rest.contains('<') || rest.contains('>')) {
        return Err(format!(
            "Use a Ctrl or Alt shortcut with a supported key: {accel}"
        ));
    }
    Ok(format!(
        "{}{}{}{}",
        if control { "<control>" } else { "" },
        if shift { "<shift>" } else { "" },
        if alt { "<alt>" } else { "" },
        key
    ))
}

pub fn set(s: &mut Settings, key: &str, value: &str) -> Result<(), String> {
    let mut next = s.clone();
    match key {
        "layout.density" => next.density = value.into(),
        "tabs.sidebar_width" => {
            next.sidebar_width = value.parse().map_err(|_| "Invalid sidebar width")?
        }
        "appearance.accent" => next.accent = value.into(),
        "new_tab.url" => next.new_tab_url = value.into(),
        "new_tab.extension" => next.new_tab_extension = value.into(),
        "toolbar.actions" => {
            next.toolbar_actions = serde_json::from_str(value).map_err(|e| e.to_string())?
        }
        "tabs.layout" => next.tab_layout = value.into(),
        "features.link_previews" => next.link_previews = parse_bool(value)?,
        "search.engine" => next.search = value.into(),
        "appearance" => next.dark = value.into(),
        "restore_tabs" => next.restore = parse_bool(value)?,
        "block_trackers" => next.block = parse_bool(value)?,
        "zoom" => next.zoom = value.parse().map_err(|_| "zoom must be a number")?,
        _ if key.starts_with("shortcuts.") => {
            let name = &key[10..];
            if !OVERRIDES.contains(&name) {
                return Err(format!("Unknown shortcut: {name}"));
            }
            if value == "default" {
                next.shortcuts.remove(name);
            } else {
                next.shortcuts.insert(name.to_owned(), value.to_owned());
            }
        }
        _ => return Err(format!("Unknown setting: {key}")),
    }
    validate(&next)?;
    *s = next;
    Ok(())
}
fn parse_bool(s: &str) -> Result<bool, String> {
    match s {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err("Use true or false".into()),
    }
}

pub fn cli(args: &[String]) -> i32 {
    match run(args) {
        Ok(value) => {
            println!("{}", serde_json::to_string_pretty(&value).unwrap());
            0
        }
        Err(e) => {
            eprintln!("{}", serde_json::json!({"error":e}));
            2
        }
    }
}
fn run(args: &[String]) -> Result<serde_json::Value, String> {
    if args == ["schema"] {
        return Ok(schema());
    }
    let initial = document()?;
    if args == ["inspect"] {
        return serde_json::to_value(initial).map_err(|e| e.to_string());
    }
    if args == ["get"] {
        return serde_json::to_value(initial.settings).map_err(|e| e.to_string());
    }
    if args.first().is_some_and(|a| a == "get") && args.len() == 2 {
        let json = serde_json::to_value(&initial.settings).unwrap();
        let key = match args[1].as_str() {
            "new_tab.extension" => "new_tab_extension",
            "layout.density" => "density",
            "tabs.sidebar_width" => "sidebar_width",
            "appearance.accent" => "accent",
            "new_tab.url" => "new_tab_url",
            "toolbar.actions" => "toolbar_actions",
            "tabs.layout" => "tab_layout",
            "features.link_previews" => "link_previews",
            "search.engine" => "search",
            "appearance" => "dark",
            "restore_tabs" => "restore",
            "block_trackers" => "block",
            "zoom" => "zoom",
            k if k.starts_with("shortcuts.") && OVERRIDES.contains(&&k[10..]) => {
                return Ok(json["shortcuts"][&k[10..]].clone())
            }
            _ => return Err("Unknown setting".into()),
        };
        return Ok(json[key].clone());
    }
    let mut expected = None;
    let mut dry_run = false;
    let mut positional = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--dry-run" => dry_run = true,
            "--if-revision" => {
                expected = Some(
                    iter.next()
                        .ok_or("Missing revision")?
                        .parse::<u64>()
                        .map_err(|_| "Invalid revision")?,
                )
            }
            _ => positional.push(arg.as_str()),
        }
    }
    let doc = config_store::transact(&path(), &initial.settings, expected, dry_run, |d| {
        match positional.as_slice() {
            ["set", key, value] => set(&mut d.settings, key, value),
            ["apply", json] => {
                let changes: std::collections::BTreeMap<String, serde_json::Value> = serde_json::from_str(json).map_err(|e| e.to_string())?;
                for (key, value) in changes {
                    let text = value.as_str().map(str::to_owned).unwrap_or_else(|| value.to_string());
                    set(&mut d.settings, &key, &text)?;
                }
                Ok(())
            }
            ["undo"] => {
                d.settings = d.history.last().ok_or("No configuration history")?.settings.clone();
                Ok(())
            }
            _ => Err("Usage: nagi config schema|get [KEY]|inspect|set KEY VALUE|apply JSON|undo [--dry-run] [--if-revision N]".into()),
        }
    })?;
    serde_json::to_value(doc).map_err(|e| e.to_string())
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
        assert!(set(&mut s, "shortcuts.search", "<Control>t").is_err());
        assert!(set(&mut s, "features.link_previews", "yes").is_err());
        assert_eq!(s.shortcuts["search"], "<Control><Alt>p");
        assert_eq!(s.tab_layout, "Left");
    }
}
