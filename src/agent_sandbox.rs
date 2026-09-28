//! Opt-in offline agent execution. Bubblewrap failure never falls back to host execution.
use std::{
    io::IsTerminal,
    os::unix::{fs::FileTypeExt, process::CommandExt},
    path::Path,
    process::Command,
};
pub fn cli(args: &[String]) -> i32 {
    match run(args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("{e}");
            2
        }
    }
}
fn run(args: &[String]) -> Result<i32, String> {
    if std::io::stdin().is_terminal()
        || std::io::stdout().is_terminal()
        || std::io::stderr().is_terminal()
    {
        return Err("Isolated agents require piped stdin/stdout/stderr through a trusted host; no terminal access".into());
    }
    let Some((separator, command)) = args.split_first() else {
        return Err("Usage: nagi agent-run -- /usr/bin/COMMAND [ARGS...]".into());
    };
    if separator != "--" || command.is_empty() {
        return Err("Supply -- followed by a command".into());
    }
    let executable = std::fs::canonicalize(&command[0]).map_err(|e| e.to_string())?;
    if !executable.starts_with("/usr/") {
        return Err(
            "Agent executable must be installed under /usr; pass source through stdin".into(),
        );
    }
    let socket = crate::control_transport::socket_path();
    if !std::fs::symlink_metadata(&socket)
        .map_err(|_| "Start Nagi with --agent-control first")?
        .file_type()
        .is_socket()
    {
        return Err("Control endpoint is not a Unix socket".into());
    }
    let binary = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut child = Command::new("/usr/bin/bwrap");
    child.args([
        "--unshare-all",
        "--disable-userns",
        "--die-with-parent",
        "--new-session",
        "--cap-drop",
        "ALL",
        "--clearenv",
        "--ro-bind",
        "/usr",
        "/usr",
        "--proc",
        "/proc",
        "--dev",
        "/dev",
        "--tmpfs",
        "/tmp",
        "--dir",
        "/home/agent",
        "--dir",
        "/run/nagi-control",
        "--dir",
        "/opt/nagi",
        "--setenv",
        "HOME",
        "/home/agent",
        "--setenv",
        "PATH",
        "/opt/nagi:/usr/bin",
        "--setenv",
        "XDG_RUNTIME_DIR",
        "/run",
        "--setenv",
        "LANG",
        "C.UTF-8",
        "--chdir",
        "/home/agent",
    ]);
    for directory in ["/bin", "/sbin", "/lib", "/lib64"] {
        if Path::new(directory).exists() {
            child.args(["--ro-bind", directory, directory]);
        }
    }
    if Path::new("/etc/ld.so.cache").exists() {
        child.args(["--ro-bind", "/etc/ld.so.cache", "/etc/ld.so.cache"]);
    }
    child.arg("--ro-bind").arg(binary).arg("/opt/nagi/nagi");
    child
        .arg("--ro-bind")
        .arg(socket)
        .arg("/run/nagi-control/browser.sock");
    child.arg("--").arg(executable).args(&command[1..]);
    // Replace the launcher so signals/lifetime belong to bubblewrap's reaper.
    Err(format!(
        "Could not start isolated agent (no fallback): {}",
        child.exec()
    ))
}
