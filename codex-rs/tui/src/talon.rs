use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

const TALON_DIR_NAME: &str = ".codex-talon";
const REQUEST_FILENAME: &str = "request.json";
const RESPONSE_FILENAME: &str = "response.json";

static STATUS_SUMMARY: Mutex<Option<String>> = Mutex::new(None);

#[derive(Debug, Clone)]
pub(crate) struct TalonPaths {
    pub request_path: PathBuf,
    pub response_path: PathBuf,
}

pub(crate) fn resolve_paths() -> Result<TalonPaths> {
    let home = dirs::home_dir().context("unable to locate home directory for Talon RPC paths")?;
    let base_dir = home.join(TALON_DIR_NAME);
    if !base_dir.exists() {
        fs::create_dir_all(&base_dir).context("failed to create ~/.codex-talon directory")?;
    }

    let request_path = base_dir.join(REQUEST_FILENAME);
    let response_path = base_dir.join(RESPONSE_FILENAME);

    Ok(TalonPaths {
        request_path,
        response_path,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct TalonRequest {
    #[serde(default)]
    pub commands: Vec<TalonCommand>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum TalonCommand {
    /// Replace the entire composer buffer with `text`. Optionally update the cursor.
    SetBuffer {
        text: String,
        #[serde(default)]
        cursor: Option<usize>,
    },
    /// Move the cursor to the provided absolute byte offset within the buffer.
    SetCursor { cursor: usize },
    /// No-op request that asks Codex to write its current state snapshot.
    GetState,
    /// Post a lightweight notification (no buffer/cursor change).
    Notify { message: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TalonResponseStatus {
    Ok,
    NoRequest,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TalonEditorState {
    pub buffer: String,
    pub cursor: usize,
    pub is_task_running: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TalonResponse {
    pub version: u32,
    pub status: TalonResponseStatus,
    pub state: TalonEditorState,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub applied: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub timestamp_ms: u128,
}

pub(crate) fn read_request(paths: &TalonPaths) -> Result<Option<TalonRequest>> {
    let Ok(raw) = fs::read_to_string(&paths.request_path) else {
        return Ok(None);
    };

    if raw.trim().is_empty() {
        return Ok(None);
    }

    let request: TalonRequest = serde_json::from_str(&raw).with_context(|| {
        format!(
            "failed to parse Talon request at {}",
            paths.request_path.display()
        )
    })?;
    Ok(Some(request))
}

pub(crate) fn remove_request(paths: &TalonPaths) -> io::Result<()> {
    match fs::remove_file(&paths.request_path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

pub(crate) fn write_response(paths: &TalonPaths, response: &TalonResponse) -> Result<()> {
    let payload =
        serde_json::to_vec_pretty(response).context("failed to serialize Talon response")?;
    fs::write(&paths.response_path, payload).with_context(|| {
        format!(
            "failed to write Talon response to {}",
            paths.response_path.display()
        )
    })
}

pub(crate) fn now_timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default()
}

pub(crate) fn set_status_summary(summary: Option<String>) {
    if let Ok(mut guard) = STATUS_SUMMARY.lock() {
        *guard = summary;
    }
}

pub(crate) fn status_summary() -> Option<String> {
    STATUS_SUMMARY.lock().ok().and_then(|guard| guard.clone())
}
