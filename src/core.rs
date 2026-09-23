use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

pub const APP_ID: &str = "io.github.tcballard.Nagi";
pub const APP_NAME: &str = "Nagi";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub search: String,
    pub restore: bool,
    pub block: bool,
    pub dark: String,
    pub zoom: f64,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            search: "DuckDuckGo".into(),
            restore: true,
            block: true,
            dark: "Theme".into(),
            zoom: 1.0,
        }
    }
}
#[derive(Clone, Serialize, Deserialize, Debug, Default, PartialEq)]
pub struct Page {
    pub url: String,
    pub title: String,
    #[serde(default)]
    pub pinned: bool,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Visit {
    pub url: String,
    pub title: String,
    pub time: u64,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct State {
    pub schema: u32,
    pub tabs: Vec<Page>,
    pub active: usize,
    pub bookmarks: Vec<Page>,
    pub history: Vec<Visit>,
    pub settings: Settings,
    pub hidden: BTreeMap<String, Vec<String>>,
    pub allowed: Vec<String>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            schema: 1,
            tabs: vec![],
            active: 0,
            bookmarks: vec![],
            history: vec![],
            settings: Settings::default(),
            hidden: BTreeMap::new(),
            allowed: vec![],
        }
    }
}
impl State {
    pub fn visit(&mut self, uri: &str, title: &str) {
        if !uri.starts_with("https://") && !uri.starts_with("http://") {
            return;
        }
        self.history.retain(|v| v.url != uri);
        self.history.insert(
            0,
            Visit {
                url: uri.into(),
                title: title.into(),
                time: now(),
            },
        );
        self.history.truncate(5000);
    }
}
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn xdg(key: &str, fallback: &str) -> PathBuf {
    std::env::var_os(key)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| "/tmp".into())).join(fallback)
        })
}
pub fn data_dir() -> PathBuf {
    xdg("XDG_DATA_HOME", ".local/share").join("nagi")
}
pub fn state_dir() -> PathBuf {
    xdg("XDG_STATE_HOME", ".local/state").join("nagi")
}
pub fn cache_dir() -> PathBuf {
    xdg("XDG_CACHE_HOME", ".cache").join("nagi")
}
pub fn origin(uri: &str) -> Option<String> {
    let u = url::Url::parse(uri).ok()?;
    if ["http", "https"].contains(&u.scheme()) {
        Some(u.origin().ascii_serialization())
    } else {
        None
    }
}
pub fn host(uri: &str) -> String {
    url::Url::parse(uri)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default()
}
/// Only explicit navigation-safe schemes are accepted. javascript/data are never
/// executed from the omnibox or an external command line.
pub fn address(input: &str, engine: &str) -> Result<String, String> {
    let s = input.trim();
    if s.is_empty() {
        return Ok("about:blank".into());
    }
    if s == "about:blank" {
        return Ok(s.into());
    }
    if s.chars().any(|c| c.is_control()) {
        return Err("The address contains control characters.".into());
    }
    if s.starts_with('/') {
        return url::Url::from_file_path(s)
            .map(|u| u.to_string())
            .map_err(|_| "Invalid file path.".into());
    }
    if let Ok(u) = url::Url::parse(s) {
        if ["http", "https", "file"].contains(&u.scheme()) {
            if ["http", "https"].contains(&u.scheme()) && u.host_str().is_none() {
                return Err("This address needs a hostname.".into());
            }
            return Ok(u.to_string());
        }
        if s.contains("://") || ["javascript", "data", "vbscript", "about"].contains(&u.scheme()) {
            return Err("Use an http, https or file address.".into());
        }
    }
    if !s.contains(char::is_whitespace)
        && (s.contains('.') || s.starts_with("localhost") || s.starts_with('['))
    {
        let prefix =
            if s.starts_with("localhost") || s.starts_with("127.") || s.starts_with("[::1]") {
                "http"
            } else {
                "https"
            };
        if let Ok(u) = url::Url::parse(&format!("{prefix}://{s}")) {
            return Ok(u.to_string());
        }
    }
    let base = match engine {
        "Google" => "https://www.google.com/search",
        "Brave" => "https://search.brave.com/search",
        _ => "https://duckduckgo.com/",
    };
    let mut u = url::Url::parse(base).unwrap();
    u.query_pairs_mut().append_pair("q", s);
    Ok(u.to_string())
}
pub fn html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
pub fn safe_filename(s: &str) -> String {
    let s: String = s
        .chars()
        .filter(|c| !c.is_control() && !['/', '\\'].contains(c))
        .collect();
    let s = s.trim_matches('.').trim();
    if s.is_empty() {
        "download".into()
    } else {
        s.chars().take(180).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation() {
        assert_eq!(
            address("example.com/a", "DuckDuckGo").unwrap(),
            "https://example.com/a"
        );
        assert_eq!(
            address("localhost:8787/a", "DuckDuckGo").unwrap(),
            "http://localhost:8787/a"
        );
        assert!(address("how does rust work", "Brave")
            .unwrap()
            .contains("q=how+does+rust+work"));
        assert!(address("javascript:alert(1)", "Google").is_err());
        assert!(address("data:text/html,hello", "Google").is_err());
        assert!(address("/tmp/a b.html", "Google")
            .unwrap()
            .contains("a%20b.html"));
    }
    #[test]
    fn origins_separate_ports_and_schemes() {
        assert_ne!(origin("http://a.test"), origin("https://a.test"));
        assert_ne!(origin("https://a.test:444"), origin("https://a.test"));
        assert_eq!(origin("file:///a"), None);
    }
    #[test]
    fn history_is_bounded_and_deduplicated() {
        let mut s = State::default();
        s.visit("https://a.test", "One");
        s.visit("https://a.test", "Two");
        s.visit("file:///private", "Private");
        assert_eq!(s.history.len(), 1);
        assert_eq!(s.history[0].title, "Two");
        for i in 0..5100 {
            s.visit(&format!("https://a.test/{i}"), "Page");
        }
        assert_eq!(s.history.len(), 5000);
    }
    #[test]
    fn filenames_cannot_escape() {
        assert_eq!(safe_filename("../../secret"), "secret");
        assert_eq!(safe_filename(".."), "download");
        assert_eq!(html("<script>\"&"), "&lt;script&gt;&quot;&amp;");
    }
}
