use serde::Serialize;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct SidecarStatus {
    pub connected: bool,
    pub port: Option<u16>,
}

struct Inner {
    child: Option<Child>,
    port: Option<u16>,
}

pub struct SidecarManager {
    inner: Mutex<Inner>,
}

impl SidecarManager {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                child: None,
                port: None,
            }),
        }
    }

    /// Starts the sidecar with `python -m main`, cwd = `sidecar/`, `PYTHONPATH=src`.
    /// Uses `sidecar/.venv/bin/python3` when present (from `uv sync` / `python -m venv`), else `python3` on PATH.
    /// Returns bound port from first stdout line `SIDECAR_PORT=<n>`.
    pub fn start(&self) -> Result<u16, String> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| format!("sidecar lock poisoned: {e}"))?;

        if let Some(p) = guard.port {
            return Ok(p);
        }

        let sidecar_dir = resolve_sidecar_dir()?;
        let pythonpath = sidecar_dir.join("src");
        let python = resolve_python_executable(&sidecar_dir);

        let mut child = Command::new(&python)
            .env("PYTHONPATH", &pythonpath)
            .args(["-m", "main"])
            .current_dir(&sidecar_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| {
                format!(
                    "failed to spawn sidecar with {:?}: {e}. \
                     Create sidecar/.venv (e.g. `cd sidecar && uv sync`) or install deps into the chosen Python.",
                    python
                )
            })?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "sidecar: missing stdout pipe".to_string())?;

        let (tx, rx) = mpsc::channel::<Result<u16, String>>();
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => {
                        let _ = tx.send(Err(
                            "sidecar stdout closed before SIDECAR_PORT line".to_string()
                        ));
                        return;
                    }
                    Ok(_) => {
                        if let Some(port) = parse_sideline(line.trim()) {
                            let _ = tx.send(Ok(port));
                            let mut drain = String::new();
                            while reader.read_line(&mut drain).unwrap_or(0) > 0 {
                                drain.clear();
                            }
                            return;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(format!("read sidecar stdout: {e}")));
                        return;
                    }
                }
            }
        });

        const START_TIMEOUT: Duration = Duration::from_secs(45);
        match rx.recv_timeout(START_TIMEOUT) {
            Ok(Ok(port)) => {
                guard.child = Some(child);
                guard.port = Some(port);
                Ok(port)
            }
            Ok(Err(e)) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(e)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(format!(
                    "timed out after {:?} waiting for SIDECAR_PORT from sidecar",
                    START_TIMEOUT
                ))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let _ = child.kill();
                let _ = child.wait();
                Err("sidecar stdout reader disconnected unexpectedly".to_string())
            }
        }
    }

    pub fn stop(&self) -> Result<(), String> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| format!("sidecar lock poisoned: {e}"))?;
        if let Some(mut c) = guard.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        guard.port = None;
        Ok(())
    }

    pub fn port(&self) -> Option<u16> {
        self.inner.lock().ok().and_then(|g| g.port)
    }
}

impl Drop for SidecarManager {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn parse_sideline(line: &str) -> Option<u16> {
    const PREFIX: &str = "SIDECAR_PORT=";
    let rest = line.strip_prefix(PREFIX)?;
    rest.trim().parse().ok()
}

fn resolve_sidecar_dir() -> Result<PathBuf, String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let from_cwd = cwd.join("sidecar");
    if sidecar_looks_valid(&from_cwd) {
        return Ok(from_cwd);
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let from_manifest = manifest_dir.join("../sidecar");
    if sidecar_looks_valid(&from_manifest) {
        return from_manifest.canonicalize().map_err(|e| e.to_string());
    }

    Err(format!(
        "sidecar directory not found (expected ./sidecar from cwd {:?} or next to src-tauri); cwd sidecar={:?}",
        cwd, from_cwd
    ))
}

fn sidecar_looks_valid(path: &Path) -> bool {
    path.is_dir() && path.join("src").join("main.py").is_file()
}

/// Prefer `sidecar/.venv` so Tauri does not rely on a global `python3` having uvicorn/FastAPI.
fn resolve_python_executable(sidecar_dir: &Path) -> PathBuf {
    if let Some(p) = venv_python(sidecar_dir) {
        return p;
    }
    PathBuf::from("python3")
}

fn venv_python(sidecar_dir: &Path) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let p = sidecar_dir.join(".venv").join("Scripts").join("python.exe");
        return p.is_file().then_some(p);
    }
    #[cfg(not(windows))]
    {
        for name in ["python3", "python"] {
            let p = sidecar_dir.join(".venv").join("bin").join(name);
            if p.is_file() {
                return Some(p);
            }
        }
        None
    }
}

fn port_tcp_reachable(port: u16) -> bool {
    use std::net::{SocketAddr, TcpStream};
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    TcpStream::connect_timeout(&addr, Duration::from_millis(600)).is_ok()
}

#[tauri::command]
pub fn get_sidecar_status(manager: State<'_, SidecarManager>) -> SidecarStatus {
    let port = manager.port();
    let connected = port.map(port_tcp_reachable).unwrap_or(false);
    SidecarStatus { connected, port }
}
