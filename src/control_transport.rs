//! Local, bounded newline-delimited JSON transport. No TCP listener.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    os::unix::{
        fs::{OpenOptionsExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub id: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}
pub struct Incoming {
    pub request: Request,
    pub reply: mpsc::Sender<Value>,
    pub received: Instant,
}
pub fn failure(id: &str, message: &str) -> Value {
    json!({"id":id,"error":message})
}
pub fn directory() -> PathBuf {
    crate::core::xdg("XDG_RUNTIME_DIR", ".cache").join("nagi-control")
}
pub fn socket_path() -> PathBuf {
    directory().join("browser.sock")
}
pub struct Server {
    pub receiver: mpsc::Receiver<Incoming>,
    stop: Arc<AtomicBool>,
    join: Option<thread::JoinHandle<()>>,
    _lock: File,
}
impl Server {
    pub fn start() -> Result<Self, String> {
        let dir = directory();
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(dir.join("server.lock"))
            .map_err(|e| e.to_string())?;
        lock.try_lock()
            .map_err(|_| "Agent control already running")?;
        let path = socket_path();
        if path.exists() {
            fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        let listener = UnixListener::bind(&path).map_err(|e| e.to_string())?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let (tx, receiver) = mpsc::sync_channel(16);
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let join = thread::spawn(move || {
            let mut workers: Vec<thread::JoinHandle<()>> = vec![];
            while !stopped.load(Ordering::Relaxed) {
                let mut i = 0;
                while i < workers.len() {
                    if workers[i].is_finished() {
                        let _ = workers.swap_remove(i).join();
                    } else {
                        i += 1;
                    }
                }
                if let Ok((stream, _)) = listener.accept() {
                    if workers.len() >= 16 {
                        drop(stream);
                        continue;
                    }
                    let sender = tx.clone();
                    let flag = stopped.clone();
                    workers.push(thread::spawn(move || {
                        let _ = serve(stream, sender, flag);
                    }));
                } else {
                    thread::sleep(Duration::from_millis(10));
                }
            }
            for worker in workers {
                let _ = worker.join();
            }
            let _ = fs::remove_file(path);
        });
        Ok(Self {
            receiver,
            stop,
            join: Some(join),
            _lock: lock,
        })
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
fn serve(
    mut stream: UnixStream,
    sender: mpsc::SyncSender<Incoming>,
    stop: Arc<AtomicBool>,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_millis(500)))?;
    stream.set_write_timeout(Some(Duration::from_millis(500)))?;
    let mut data = vec![];
    let mut reader = BufReader::new(&stream);
    while data.len() <= 65536 {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return Ok(());
        }
        let count = available
            .iter()
            .position(|b| *b == b'\n')
            .map(|n| n + 1)
            .unwrap_or(available.len())
            .min(65537 - data.len());
        let done = available[count - 1] == b'\n';
        data.extend_from_slice(&available[..count]);
        reader.consume(count);
        if done {
            break;
        }
    }
    let result =
        if data.len() > 65536 {
            failure("", "Request exceeds 64 KiB")
        } else {
            match serde_json::from_slice::<Request>(&data) {
                Ok(request) if !request.id.is_empty() && request.id.len() <= 128 => {
                    let id = request.id.clone();
                    let (reply, response) = mpsc::channel();
                    if sender
                        .try_send(Incoming {
                            request,
                            reply,
                            received: Instant::now(),
                        })
                        .is_err()
                    {
                        failure(&id, "Browser busy")
                    } else {
                        let deadline = Instant::now() + Duration::from_secs(17);
                        loop {
                            if stop.load(Ordering::Relaxed) {
                                break failure(&id, "Agent control stopped");
                            }
                            match response.recv_timeout(Duration::from_millis(100)) {
                                Ok(value) => break value,
                                Err(mpsc::RecvTimeoutError::Disconnected) => {
                                    break failure(&id, "Browser disconnected")
                                }
                                Err(_) if Instant::now() >= deadline => break failure(
                                    &id,
                                    "Request timed out; inspect state before retrying a mutation",
                                ),
                                Err(_) => {}
                            }
                        }
                    }
                }
                _ => failure("", "Invalid request; expected id, method, params"),
            }
        };
    writeln!(stream, "{result}")
}
pub fn cli(args: &[String]) -> i32 {
    let result = (|| {
        if args.is_empty() {
            return Err("Usage: nagi browser METHOD [JSON_PARAMS] [REQUEST_ID]".to_string());
        }
        if args.len() > 3 {
            return Err("Too many arguments".into());
        }
        let id = args.get(2).cloned().unwrap_or_else(|| {
            format!(
                "{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            )
        });
        let params = args
            .get(1)
            .map(|s| serde_json::from_str(s))
            .transpose()
            .map_err(|e| e.to_string())?
            .unwrap_or(json!({}));
        let request = Request {
            id,
            method: args[0].clone(),
            params,
        };
        let mut stream = UnixStream::connect(socket_path()).map_err(|_| "Agent control is unavailable. Start Nagi with --agent-control (unless in safe mode).".to_string())?;
        stream
            .set_read_timeout(Some(Duration::from_secs(20)))
            .map_err(|e| e.to_string())?;
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .map_err(|e| e.to_string())?;
        writeln!(stream, "{}", serde_json::to_string(&request).unwrap())
            .map_err(|e| e.to_string())?;
        let mut line = String::new();
        BufReader::new(stream)
            .read_line(&mut line)
            .map_err(|e| e.to_string())?;
        let value: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        Ok(value)
    })();
    match result {
        Ok(value) if value.get("error").is_none() => {
            println!("{value}");
            0
        }
        Ok(value) => {
            eprintln!("{value}");
            2
        }
        Err(e) => {
            eprintln!("{}", failure("", &e));
            2
        }
    }
}
