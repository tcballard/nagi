//! Versioned transactions shared by CLI and GTK. The lock inode is never replaced.
use crate::{config, core::Settings};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub schema: u32,
    pub revision: u64,
    pub settings: Settings,
    pub history: Vec<Snapshot>,
    #[serde(default)]
    pub audit: Vec<Audit>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Audit {
    pub timestamp: u64,
    pub proposal: String,
    pub reason: String,
    pub outcome: String,
    pub from_revision: u64,
    pub to_revision: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub revision: u64,
    pub settings: Settings,
}
impl Document {
    pub fn initial(settings: Settings) -> Self {
        Self {
            schema: 2,
            revision: 0,
            settings,
            history: vec![],
            audit: vec![],
        }
    }
}
pub fn read(path: &Path) -> Result<Option<Document>, String> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    if bytes.len() > 1024 * 1024 {
        return Err("Settings exceed 1 MiB; preserved".into());
    }
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("Invalid settings; preserved: {e}"))?;
    let doc: Document = if value.get("schema").is_some() {
        serde_json::from_value(value)
            .map_err(|e| format!("Invalid settings envelope; preserved: {e}"))?
    } else {
        Document::initial(
            serde_json::from_value(value)
                .map_err(|e| format!("Invalid legacy settings; preserved: {e}"))?,
        )
    };
    if !matches!(doc.schema, 1 | 2) {
        return Err("Unsupported settings schema; preserved".into());
    }
    if doc.history.len() > 32 {
        return Err("Invalid settings history; preserved".into());
    }
    config::validate(&doc.settings)?;
    for snapshot in &doc.history {
        config::validate(&snapshot.settings)?;
    }
    Ok(Some(doc))
}
pub fn transact<F>(
    path: &Path,
    fallback: &Settings,
    expected: Option<u64>,
    dry_run: bool,
    edit: F,
) -> Result<Document, String>
where
    F: FnOnce(&mut Document) -> Result<(), String>,
{
    transact_audited(path, fallback, expected, dry_run, None, edit)
}
pub fn transact_audited<F>(
    path: &Path,
    fallback: &Settings,
    expected: Option<u64>,
    dry_run: bool,
    record: Option<(&str, &str, &str)>,
    edit: F,
) -> Result<Document, String>
where
    F: FnOnce(&mut Document) -> Result<(), String>,
{
    let dir = path.parent().ok_or("Missing settings directory")?;
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(path.with_extension("lock"))
        .map_err(|e| e.to_string())?;
    lock.try_lock()
        .map_err(|_| "Settings busy; retry the transaction".to_string())?;
    let mut doc = read(path)?.unwrap_or_else(|| Document::initial(fallback.clone()));
    if expected.is_some_and(|r| r != doc.revision) {
        return Err(format!(
            "Revision conflict: current revision is {}",
            doc.revision
        ));
    }
    let previous = Snapshot {
        revision: doc.revision,
        settings: doc.settings.clone(),
    };
    edit(&mut doc)?;
    config::validate(&doc.settings)?;
    let changed = doc.settings != previous.settings;
    if !changed && record.is_none() {
        return Ok(doc);
    }
    let from_revision = doc.revision;
    if changed {
        doc.history.push(previous);
        if doc.history.len() > 32 {
            doc.history.remove(0);
        }
        doc.revision = doc.revision.checked_add(1).ok_or("Revision exhausted")?;
    }
    if !dry_run {
        let (proposal, reason, outcome) =
            record.unwrap_or(("owner", "Owner configuration transaction", "applied"));
        doc.audit.push(Audit {
            timestamp: crate::core::now(),
            proposal: proposal.into(),
            reason: reason.into(),
            outcome: outcome.into(),
            from_revision,
            to_revision: doc.revision,
        });
        doc.schema = 2;
        if serde_json::to_vec_pretty(&doc)
            .map_err(|e| e.to_string())?
            .len()
            > 1024 * 1024
        {
            return Err(
                "Settings audit is full; preserve and archive it before further changes".into(),
            );
        }
        atomic_json(path, &doc)?;
    }
    Ok(doc)
}
pub fn atomic_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    let result = (|| {
        let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)
            .map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        fs::rename(&tmp, path).map_err(|e| e.to_string())?;
        File::open(path.parent().ok_or("Missing parent")?)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn audit_and_settings_are_one_commit_and_v1_migrates() {
        let dir = std::env::temp_dir().join(format!("nagi-audit-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        fs::write(
            &path,
            br#"{"schema":1,"revision":4,"settings":{},"history":[]}"#,
        )
        .unwrap();
        let defaults = Settings::default();
        let proposed = transact_audited(
            &path,
            &defaults,
            Some(4),
            false,
            Some(("session:1", "Read comfortably", "proposed")),
            |_| Ok(()),
        )
        .unwrap();
        assert_eq!(proposed.schema, 2);
        assert_eq!(proposed.revision, 4);
        let applied = transact_audited(
            &path,
            &defaults,
            Some(4),
            false,
            Some(("session:1", "Read comfortably", "applied")),
            |d| config::set(&mut d.settings, "zoom", "1.2"),
        )
        .unwrap();
        let persisted = read(&path).unwrap().unwrap();
        assert_eq!(persisted.settings, applied.settings);
        assert_eq!(persisted.audit.len(), 2);
        assert_eq!(persisted.audit[1].to_revision, 5);
        assert_eq!(persisted.audit[1].from_revision, 4);
        let before = fs::read(&path).unwrap();
        assert!(transact_audited(
            &path,
            &defaults,
            Some(4),
            false,
            Some(("session:1", "Replay", "applied")),
            |_| Ok(())
        )
        .is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        let oversized = "x".repeat(1024 * 1024);
        assert!(transact_audited(
            &path,
            &defaults,
            Some(5),
            false,
            Some(("session:2", &oversized, "proposed")),
            |_| Ok(())
        )
        .is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn migration_conflict_dry_run_undo_and_unknown_schema() {
        let dir = std::env::temp_dir().join(format!("nagi-config-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        fs::write(&path, br#"{"search":"Google"}"#).unwrap();
        let original = fs::read(&path).unwrap();
        let defaults = Settings::default();
        let edit = |d: &mut Document| config::set(&mut d.settings, "tabs.layout", "Left");
        let preview = transact(&path, &defaults, Some(0), true, edit).unwrap();
        assert_eq!(preview.revision, 1);
        assert_eq!(fs::read(&path).unwrap(), original);
        let changed = transact(&path, &defaults, Some(0), false, edit).unwrap();
        assert_eq!(changed.settings.search, "Google");
        assert!(transact(&path, &defaults, Some(0), false, edit).is_err());
        let restored = transact(&path, &defaults, Some(1), false, |d| {
            d.settings = d.history[0].settings.clone();
            Ok(())
        })
        .unwrap();
        assert_eq!(restored.settings.tab_layout, "Top");
        assert_eq!(restored.revision, 2);
        fs::write(
            &path,
            br#"{"schema":99,"revision":0,"settings":{},"history":[]}"#,
        )
        .unwrap();
        let unknown = fs::read(&path).unwrap();
        assert!(transact(&path, &defaults, None, false, edit).is_err());
        assert_eq!(fs::read(&path).unwrap(), unknown);
        fs::remove_dir_all(dir).unwrap();
    }
}
