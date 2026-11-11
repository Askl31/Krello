//! Helper process management: materialize the embedded Swift binary and
//! speak the JSON-lines protocol.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use crate::error::{Error, Result};

#[derive(Debug, serde::Serialize)]
pub struct HelperRequest<'a> {
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<&'a str>,
    pub prompt: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

#[derive(Debug, serde::Deserialize)]
pub struct HelperResponse {
    pub id: u64,
    pub ok: bool,
    pub text: Option<String>,
    pub error: Option<String>,
}

pub struct HelperSession {
    child: Mutex<Option<Child>>,
}

/// Materialize the embedded helper once per process into a private temp dir.
pub fn materialize_helper() -> Result<Option<PathBuf>> {
    let bytes = krello_ai_helper::helper_binary();
    if bytes.is_empty() {
        return Ok(None);
    }
    let dir = std::env::temp_dir().join(format!("krello-ai-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("krello-ai-helper");
    if !path.exists() {
        std::fs::write(&path, bytes)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
        }
    }
    Ok(Some(path))
}

impl HelperSession {
    /// Infallible: the helper process is spawned per request, nothing can
    /// fail at construction time (panicking here used to crash the server
    /// on a corrupted environment).
    pub fn new() -> Self {
        Self::default()
    }

    fn ensure_child(&self, helper: &PathBuf) -> Result<(std::process::ChildStdin, BufReader<std::process::ChildStdout>)> {
        // We create a fresh child per request: the helper is short-lived and
        // cheap to start, which avoids all protocol locking complexity.
        let mut cmd = Command::new(helper);
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        let child = cmd.spawn().map_err(|e| Error::Ai(format!("impossible de lancer le helper: {e}")))?;
        let mut child = child;
        let stdin = child.stdin.take().ok_or_else(|| Error::Ai("stdin indisponible".into()))?;
        let stdout = child.stdout.take().ok_or_else(|| Error::Ai("stdout indisponible".into()))?;
        {
            let mut guard = self.child.lock().map_err(|_| Error::Lock)?;
            // reap previous child if any
            if let Some(old) = guard.as_mut() {
                let _ = old.kill();
                let _ = old.wait();
            }
            *guard = Some(child);
        }
        Ok((stdin, BufReader::new(stdout)))
    }

    /// Ask availability. Returns (available, reason_or_none).
    pub fn status(&self, helper: &PathBuf) -> (bool, Option<String>) {
        let Ok((stdin, mut reader)) = self.ensure_child(helper) else {
            return (false, Some("helper_launch_failed".into()));
        };
        // The helper prints its status line immediately.
        let _ = stdin; // keep stdin open during status read? dropping it ends the loop; read first line first
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => return (false, Some("helper_no_response".into())),
            Ok(_) => {}
        }
        match serde_json::from_str::<serde_json::Value>(&line) {
            Ok(v) => {
                let status = v.get("status").and_then(|s| s.as_str()).unwrap_or("unavailable:unknown");
                if status == "available" {
                    (true, None)
                } else {
                    let reason = status.strip_prefix("unavailable:").unwrap_or("unknown").to_string();
                    (false, Some(reason))
                }
            }
            Err(_) => (false, Some("helper_bad_status".into())),
        }
        // stdin dropped here → helper exits cleanly after answering.
    }

    pub fn generate(
        &self,
        helper: &PathBuf,
        system: Option<&str>,
        prompt: &str,
        max_tokens: Option<u32>,
    ) -> Result<String> {
        let (mut stdin, mut reader) = self.ensure_child(helper)?;
        // Read the status line first.
        let mut status_line = String::new();
        reader
            .read_line(&mut status_line)
            .map_err(|e| Error::Ai(format!("helper injoignable: {e}")))?;
        let status: serde_json::Value = serde_json::from_str(&status_line)
            .map_err(|_| Error::Ai("réponse de statut illisible".into()))?;
        if status.get("status").and_then(|s| s.as_str()) != Some("available") {
            let reason = status
                .get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("unavailable")
                .to_string();
            return Err(Error::Ai(reason));
        }

        let id = next_id();
        let req = serde_json::json!({
            "id": id,
            "system": system,
            "prompt": prompt,
            "max_tokens": max_tokens,
        });
        writeln!(stdin, "{req}").map_err(|e| Error::Ai(format!("écriture: {e}")))?;
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(|e| Error::Ai(format!("lecture: {e}")))?;
        let resp: HelperResponse = serde_json::from_str(&line)
            .map_err(|_| Error::Ai("réponse illisible".into()))?;
        if resp.id != id {
            return Err(Error::Ai("id de réponse incohérent".into()));
        }
        if resp.ok {
            resp.text.ok_or_else(|| Error::Ai("réponse vide".into()))
        } else {
            Err(Error::Ai(resp.error.unwrap_or_else(|| "erreur helper".into())))
        }
    }
}

impl Drop for HelperSession {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.child.lock() {
            if let Some(mut c) = guard.take() {
                let _ = c.kill();
                let _ = c.wait();
            }
        }
    }
}

impl Default for HelperSession {
    fn default() -> Self {
        Self {
            child: Mutex::new(None),
        }
    }
}

fn next_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static ID: AtomicU64 = AtomicU64::new(1);
    ID.fetch_add(1, Ordering::Relaxed)
}
