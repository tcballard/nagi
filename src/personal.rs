//! Portable preferences contain no browser records or credentials.
use crate::{config, config_store, core::Settings};
use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub schema: u32,
    pub name: String,
    pub settings: Settings,
}
pub fn http_url(value: &str) -> Result<(), String> {
    if value.len() > 8192 { return Err("URL exceeds 8192 bytes".into()); }
    let url = url::Url::parse(value).map_err(|_| "Expected an absolute HTTP(S) URL")?;
    if !["http", "https"].contains(&url.scheme()) || url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() {
        return Err("Expected HTTP(S) URL without embedded credentials".into());
    }
    Ok(())
}
pub fn cli(args: &[String]) -> i32 {
    let result = (|| {
        match args {
            [command, name] if command == "export" => {
                let profile = Profile { schema: 1, name: name.clone(), settings: config::document()?.settings };
                serde_json::to_value(profile).map_err(|e| e.to_string())
            }
            [command, rest @ ..] if command == "apply" || command == "check" => {
                let mut bytes = vec![];
                std::io::stdin().take(1024 * 1024 + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
                if bytes.len() > 1024 * 1024 { return Err("Profile exceeds 1 MiB".into()); }
                let profile: Profile = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                if profile.schema != 1 { return Err("Unsupported profile schema".into()); }
                config::validate(&profile.settings)?;
                let expected = match rest {
                    [] => None,
                    [flag, value] if flag == "--if-revision" => Some(value.parse::<u64>().map_err(|_| "Invalid revision")?),
                    _ => return Err("Expected --if-revision N".into()),
                };
                let initial = config::document()?;
                let doc = config_store::transact(&config::path(), &initial.settings, expected, command == "check", |d| {
                    d.settings = profile.settings;
                    Ok(())
                })?;
                serde_json::to_value(doc).map_err(|e| e.to_string())
            }
            _ => Err("Usage: nagi profile export NAME | check | apply [--if-revision N]; check/apply read JSON from stdin".into()),
        }
    })();
    match result {
        Ok(value) => { println!("{}", serde_json::to_string_pretty(&value).unwrap()); 0 }
        Err(e) => { eprintln!("{}", serde_json::json!({"error":e})); 2 }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profile_validation_and_safe_url_boundary() {
        for value in ["file:///etc/passwd", "javascript:alert(1)", "https://user:password@example.com", "relative"] { assert!(http_url(value).is_err()); }
        assert!(http_url("https://example.com/start").is_ok());
        let mut settings = Settings::default();
        assert!(config::set(&mut settings, "appearance.accent", "red; }").is_err());
        assert!(config::set(&mut settings, "tabs.sidebar_width", "9999").is_err());
        assert!(config::set(&mut settings, "toolbar.actions", "[\"quit\"]").is_err());
        config::set(&mut settings, "layout.density", "Compact").unwrap();
    }
}
