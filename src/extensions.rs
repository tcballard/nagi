//! User-owned, versioned extension manifests. Approval is tied to exact contents.
use crate::{
    config, config_store,
    core::{origin, xdg},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Read,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::PathBuf,
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub api: u32,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub commands: Vec<Command>,
    #[serde(default)]
    pub sidebar_html: String,
    #[serde(default)]
    pub new_tab_html: String,
    #[serde(default)]
    pub sites: Vec<Site>,
    #[serde(default)]
    pub events: Vec<Event>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub id: String,
    pub label: String,
    pub action: Action,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Open {
        url: String,
    },
    Sidebar,
    Configure {
        changes: BTreeMap<String, serde_json::Value>,
    },
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Site {
    pub origin: String,
    #[serde(default)]
    pub css: String,
    #[serde(default)]
    pub script: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub kind: String,
    pub origin: String,
    pub message: String,
}
#[derive(Clone)]
pub struct Installed {
    pub manifest: Manifest,
    pub digest: String,
    pub enabled: bool,
}
pub fn root() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config").join("nagi/extensions")
}
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 48
        && id
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}
pub fn validate(m: &Manifest) -> Result<(), String> {
    if m.api != 1 {
        return Err("Unsupported extension API; requires 1".into());
    }
    if !valid_id(&m.id) || m.name.is_empty() || m.name.len() > 80 || m.description.len() > 500 {
        return Err("Invalid extension identity".into());
    }
    if m.commands.len() > 16
        || m.sites.len() > 16
        || m.events.len() > 16
        || m.sidebar_html.len() > 65536
        || m.new_tab_html.len() > 65536
    {
        return Err("Extension exceeds resource limits".into());
    }
    let mut ids = std::collections::HashSet::new();
    for command in &m.commands {
        if !valid_id(&command.id) || !ids.insert(&command.id) || command.label.len() > 80 {
            return Err("Invalid or duplicate command".into());
        }
        match &command.action {
            Action::Open { url } => crate::personal::http_url(url)?,
            Action::Sidebar if m.sidebar_html.is_empty() => {
                return Err("Sidebar command has no sidebar".into())
            }
            Action::Sidebar => {}
            Action::Configure { changes } => {
                let mut settings = crate::core::Settings::default();
                for (key, value) in changes {
                    config::set(
                        &mut settings,
                        key,
                        &value
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| value.to_string()),
                    )?;
                }
            }
        }
    }
    for site in &m.sites {
        if origin(&site.origin).as_deref() != Some(site.origin.as_str())
            || site.css.len() > 65536
            || site.script.len() > 65536
        {
            return Err(
                "Site hooks require exact HTTP(S) origins and at most 64 KiB per script/style"
                    .into(),
            );
        }
    }
    for event in &m.events {
        if !["navigation.finished", "download.finished"].contains(&event.kind.as_str())
            || origin(&event.origin).as_deref() != Some(event.origin.as_str())
            || event.message.len() > 300
        {
            return Err("Invalid event hook".into());
        }
    }
    Ok(())
}
fn grants() -> Result<BTreeMap<String, String>, String> {
    match fs::read(root().join("grants.json")) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(e) => Err(e.to_string()),
    }
}
pub fn load(id: &str) -> Result<Installed, String> {
    if !valid_id(id) {
        return Err("Invalid extension ID".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(root().join(id).join("manifest.json"))
        .map_err(|e| e.to_string())?
        .take(262145)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 262144 {
        return Err("Manifest exceeds 256 KiB".into());
    }
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    validate(&manifest)?;
    if manifest.id != id {
        return Err("Manifest ID differs from directory".into());
    }
    let digest = gtk::glib::compute_checksum_for_data(gtk::glib::ChecksumType::Sha256, &bytes)
        .ok_or("Could not hash extension")?
        .to_string();
    let enabled = grants()?.get(id) == Some(&digest);
    Ok(Installed {
        manifest,
        digest,
        enabled,
    })
}
pub fn fingerprint() -> Vec<(PathBuf, Option<std::time::SystemTime>, u64)> {
    let mut paths = vec![root().join("grants.json")];
    if let Ok(entries) = fs::read_dir(root()) {
        let mut dirs: Vec<_> = entries
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .map(|e| e.path().join("manifest.json"))
            .collect();
        dirs.sort();
        dirs.truncate(64);
        paths.extend(dirs);
    }
    paths
        .into_iter()
        .map(|path| {
            let meta = fs::metadata(&path).ok();
            let time = meta.as_ref().and_then(|m| m.modified().ok());
            let size = meta.map(|m| m.len()).unwrap_or(0);
            (path, time, size)
        })
        .collect()
}
pub fn list() -> Vec<Result<Installed, String>> {
    let Ok(entries) = fs::read_dir(root()) else {
        return vec![];
    };
    let mut ids: Vec<_> = entries
        .flatten()
        .filter_map(|e| {
            e.file_type()
                .ok()
                .filter(|t| t.is_dir())
                .map(|_| e.file_name().to_string_lossy().to_string())
        })
        .collect();
    ids.sort();
    ids.truncate(64);
    ids.into_iter().map(|id| load(&id)).collect()
}
pub fn grant(id: &str, digest: Option<&str>) -> Result<(), String> {
    if !valid_id(id) {
        return Err("Invalid extension ID".into());
    }
    fs::create_dir_all(root()).map_err(|e| e.to_string())?;
    fs::set_permissions(root(), fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(root().join("grants.lock"))
        .map_err(|e| e.to_string())?;
    lock.try_lock().map_err(|_| "Extension grants busy")?;
    let mut approved = grants()?;
    if let Some(digest) = digest {
        if load(id)?.digest != digest {
            return Err("Extension changed during approval; inspect and approve again".into());
        }
        approved.insert(id.into(), digest.into());
    } else {
        approved.remove(id);
    }
    config_store::atomic_json(&root().join("grants.json"), &approved)
}
pub fn cli(args: &[String]) -> i32 {
    let result = (|| -> Result<serde_json::Value, String> {
        match args {
            [command] if command == "schema" => Ok(serde_json::json!({"api":1,"commands":["open","sidebar","configure"],"events":["navigation.finished","download.finished"],"panels":"isolated ephemeral WebKit with no network or native bridge","sites":"exact-origin CSS and isolated-world JavaScript; requires native approval","max_extensions":64})),
            [command] if command == "list" => Ok(serde_json::json!(list().into_iter().map(|entry| match entry { Ok(e) => serde_json::json!({"id":e.manifest.id,"name":e.manifest.name,"digest":e.digest,"enabled":e.enabled}), Err(e) => serde_json::json!({"error":e}) }).collect::<Vec<_>>())),
            [command] if command == "install" || command == "check" => {
                let mut bytes = Vec::new();
                std::io::stdin().take(262145).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
                if bytes.len()>262144 { return Err("Manifest exceeds 256 KiB".into()); }
                let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                validate(&manifest)?;
                if command == "install" {
                    let dir = root().join(&manifest.id);
                    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
                    let lock = OpenOptions::new().read(true).write(true).create(true).truncate(false).mode(0o600).open(dir.join("install.lock")).map_err(|e| e.to_string())?;
                    lock.try_lock().map_err(|_| "Extension install busy")?;
                    grant(&manifest.id, None)?;
                    config_store::atomic_json(&dir.join("manifest.json"), &manifest)?;
                }
                Ok(serde_json::json!({"id":manifest.id,"valid":true,"enabled":false}))
            }
            [command, id] if command == "disable" => { grant(id, None)?; Ok(serde_json::json!({"disabled":id})) }
            _ => Err("Usage: nagi extension schema|list|check|install|disable ID; check/install read manifest JSON from stdin. Enable in Nagi's Extensions panel.".into()),
        }
    })();
    match result {
        Ok(v) => {
            println!("{v}");
            0
        }
        Err(e) => {
            eprintln!("{}", serde_json::json!({"error":e}));
            2
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manifest_boundaries() {
        assert!(!valid_id("../escape"));
        assert!(!valid_id("a/b"));
        assert!(valid_id("research"));
        let mut m: Manifest = serde_json::from_str(r#"{"api":1,"id":"research","name":"Research","sites":[{"origin":"https://example.com","css":"body { font-size: 18px; }"}]}"#).unwrap();
        validate(&m).unwrap();
        m.sites[0].origin = "https://example.com/path".into();
        assert!(validate(&m).is_err());
        m.api = 2;
        assert!(validate(&m).is_err());
    }
}
