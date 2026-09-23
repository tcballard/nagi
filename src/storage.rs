use crate::core::State;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
};

pub fn read(path: &Path) -> Result<State, String> {
    if !path.exists() {
        return Ok(State::default());
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let state: State = serde_json::from_slice(&bytes).map_err(|e| {
        format!("Saved data could not be read: {e}. The original file has been preserved.")
    })?;
    if state.schema != 1 {
        return Err(format!(
            "Data schema {} is unsupported. The original file has been preserved.",
            state.schema
        ));
    }
    Ok(state)
}
pub fn atomic_write(path: &Path, state: &State) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Missing parent"))?;
    fs::create_dir_all(parent)?;
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
    let temp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec(state)?;
    let mut f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&temp)?;
    f.write_all(&bytes)?;
    f.sync_all()?;
    fs::rename(&temp, path)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}
/// One serial writer owns disk writes, so an older snapshot can never win a race.
/// Dropping the sender drains pending saves; joining it completes final shutdown.
pub struct Writer {
    sender: Option<mpsc::Sender<State>>,
    join: Option<thread::JoinHandle<()>>,
    errors: mpsc::Receiver<String>,
}
impl Writer {
    pub fn new(path: PathBuf) -> Self {
        let (tx, rx) = mpsc::channel::<State>();
        let (error_tx, errors) = mpsc::channel();
        let join = thread::spawn(move || {
            while let Ok(mut state) = rx.recv() {
                while let Ok(newer) = rx.try_recv() {
                    state = newer;
                }
                if let Err(e) = atomic_write(&path, &state) {
                    let _ = error_tx.send(format!("Could not save browser data: {e}"));
                }
            }
        });
        Self {
            sender: Some(tx),
            join: Some(join),
            errors,
        }
    }
    pub fn save(&self, state: State) {
        if let Some(tx) = &self.sender {
            let _ = tx.send(state);
        }
    }
    pub fn error(&self) -> Option<String> {
        self.errors.try_recv().ok()
    }
}
impl Drop for Writer {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn final_snapshot_survives_reopen() {
        let dir = std::env::temp_dir().join(format!("nagi-test-{}", std::process::id()));
        let path = dir.join("state.json");
        {
            let w = Writer::new(path.clone());
            for i in 0..100 {
                let s = State {
                    active: i,
                    ..State::default()
                };
                w.save(s);
            }
        }
        assert_eq!(read(&path).unwrap().active, 99);
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::write(&path, b"{broken").unwrap();
        assert!(read(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"{broken");
        fs::write(&path, b"{\"schema\":2}").unwrap();
        assert!(read(&path).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
