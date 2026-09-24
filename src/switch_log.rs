//! Explicit, local-only records of departures from Nagi.
use crate::core;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{self, BufRead, BufReader, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
};

#[derive(Debug, Serialize, Deserialize)]
struct Switch {
    timestamp: u64,
    site_or_task: String,
    reason: String,
    nagi_version: String,
}

fn path() -> std::path::PathBuf {
    core::data_dir().join("switches.jsonl")
}

fn append(path: &Path, site: &str, reason: &str, now: u64) -> Result<(), String> {
    if site.trim().is_empty() || reason.trim().is_empty() {
        return Err("Site/task and reason must both be non-empty".into());
    }
    if site.len() > 512 || reason.len() > 2048 {
        return Err("Site/task or reason too long".into());
    }
    let parent = path.parent().ok_or("Missing data directory")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|e| e.to_string())?;
    // A single append of a bounded JSON line avoids interleaved records from
    // concurrent prompts. Lock also serializes with any future writers.
    file.lock().map_err(|e| e.to_string())?;
    let row = Switch {
        timestamp: now,
        site_or_task: site.trim().into(),
        reason: reason.trim().into(),
        nagi_version: core::VERSION.into(),
    };
    let mut bytes = serde_json::to_vec(&row).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    file.write_all(&bytes).map_err(|e| e.to_string())
}

fn read(path: &Path) -> Result<Vec<Switch>, String> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e.to_string()),
    };
    BufReader::new(file)
        .lines()
        .enumerate()
        .map(|(index, line)| {
            let line = line.map_err(|e| e.to_string())?;
            serde_json::from_str::<Switch>(&line)
                .map_err(|e| format!("Switch log line {}: {e}", index + 1))
        })
        .collect()
}

fn period(s: &str) -> Result<u64, String> {
    let (digits, scale) = if let Some(d) = s.strip_suffix('d') {
        (d, 86400_u64)
    } else if let Some(h) = s.strip_suffix('h') {
        (h, 3600)
    } else {
        return Err("--since expects a number of days or hours, such as 7d or 24h".into());
    };
    let count = digits.parse::<u64>().map_err(|_| "Invalid --since value")?;
    count
        .checked_mul(scale)
        .ok_or("--since value is too large".into())
}

fn summary(rows: &[Switch], since: u64, markdown: bool) -> String {
    let included: Vec<_> = rows.iter().filter(|r| r.timestamp >= since).collect();
    let mut by_site = BTreeMap::<String, usize>::new();
    let mut by_word = BTreeMap::<String, usize>::new();
    let mut causes = BTreeMap::<(String, String), usize>::new();
    for row in &included {
        *by_site.entry(row.site_or_task.clone()).or_default() += 1;
        *causes
            .entry((row.site_or_task.clone(), row.reason.clone()))
            .or_default() += 1;
        for word in row
            .reason
            .split(|c: char| !c.is_alphanumeric())
            .map(str::to_lowercase)
            .filter(|w| w.len() >= 3)
            .collect::<std::collections::BTreeSet<_>>()
        {
            *by_word.entry(word).or_default() += 1;
        }
    }
    fn ranked<K: Ord>(counts: BTreeMap<K, usize>) -> Vec<(K, usize)> {
        let mut values: Vec<_> = counts.into_iter().collect();
        values.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        values
    }
    let mut out = if markdown {
        format!("# Nagi browser switches\n\nSwitches: {}\n", included.len())
    } else {
        format!("Nagi browser switches: {}\n", included.len())
    };
    for (title, values) in [
        ("By site/task", ranked(by_site)),
        ("By reason keyword", ranked(by_word)),
    ] {
        out.push_str(&format!("\n{title}\n"));
        for (key, count) in values {
            out.push_str(&format!("  {count:>3}  {}\n", safe(&key, markdown)));
        }
    }
    out.push_str("\nTop 10 causes\n");
    for ((site, reason), count) in ranked(causes).into_iter().take(10) {
        out.push_str(&format!(
            "  {count:>3}  {} — {}\n",
            safe(&site, markdown),
            safe(&reason, markdown)
        ));
    }
    out
}

fn safe(value: &str, markdown: bool) -> String {
    let value = value
        .replace('\n', " ")
        .replace('\r', " ")
        .replace('\t', " ");
    if markdown {
        value.replace('|', "\\|")
    } else {
        value
    }
}

fn prompt(label: &str) -> Result<String, String> {
    eprint!("{label}: ");
    io::stderr().flush().map_err(|e| e.to_string())?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| e.to_string())?;
    Ok(input.trim().into())
}

fn run(args: &[String]) -> Result<String, String> {
    if args.first().is_some_and(|a| a == "--summary") {
        let mut since = 7 * 86400;
        let mut markdown = false;
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--markdown" => markdown = true,
                "--since" => {
                    i += 1;
                    since = period(args.get(i).ok_or("Missing value for --since")?)?;
                }
                _ => return Err(format!("Unknown summary option: {}", args[i])),
            }
            i += 1;
        }
        return Ok(summary(
            &read(&path())?,
            core::now().saturating_sub(since),
            markdown,
        ));
    }
    let (site, reason) = match args {
        [] => (prompt("Site or task")?, prompt("Reason")?),
        [site, reason] if !site.starts_with('-') => (site.clone(), reason.clone()),
        _ => return Err(
            "Usage: nagi log-switch [SITE_OR_TASK REASON] | --summary [--since 7d] [--markdown]"
                .into(),
        ),
    };
    append(&path(), &site, &reason, core::now())?;
    Ok("Switch logged locally.\n".into())
}

pub fn cli(args: &[String]) -> i32 {
    match run(args) {
        Ok(output) => {
            print!("{output}");
            0
        }
        Err(error) => {
            eprintln!("{error}");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn append_filter_rank_and_reject_invalid() {
        let root = std::env::temp_dir().join(format!("nagi-switch-test-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("switches.jsonl");
        let _ = fs::remove_file(&path);
        append(&path, "GitHub", "Login broken", 10).unwrap();
        append(&path, "GitHub", "Login broken", 20).unwrap();
        append(&path, "Docs", "Slow load", 30).unwrap();
        assert!(append(&path, "", "broken", 40).is_err());
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let output = summary(&read(&path).unwrap(), 15, true);
        assert!(output.contains("Switches: 2"));
        assert!(output.contains("GitHub — Login broken"));
        assert!(!output.contains("Switches: 3"));
        assert!(period("7d").unwrap() == 604800);
        assert!(period("2w").is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
