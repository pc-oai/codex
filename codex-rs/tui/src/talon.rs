use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Context;
use anyhow::Result;
use codex_protocol::openai_models::ReasoningEffort;
use serde::Deserialize;
use serde::Serialize;

const TALON_DIR_NAME: &str = ".codex-talon";
const REQUEST_FILENAME: &str = "request.json";
const RESPONSE_FILENAME: &str = "response.json";
const STATE_FILENAME: &str = "state.json";

static STATUS_SUMMARY: Mutex<Option<String>> = Mutex::new(None);

#[derive(Debug, Clone)]
pub(crate) struct TalonPaths {
    pub request_path: PathBuf,
    pub response_path: PathBuf,
    pub state_path: PathBuf,
}

pub(crate) fn resolve_paths() -> Result<TalonPaths> {
    let home = dirs::home_dir().context("unable to locate home directory for Talon RPC paths")?;
    resolve_paths_in_dir(home.join(TALON_DIR_NAME))
}

pub(crate) fn resolve_session_paths(session_id: &str) -> Result<TalonPaths> {
    let home = dirs::home_dir().context("unable to locate home directory for Talon RPC paths")?;
    resolve_paths_in_dir(home.join(TALON_DIR_NAME).join(session_id))
}

fn resolve_paths_in_dir(base_dir: PathBuf) -> Result<TalonPaths> {
    if !base_dir.exists() {
        fs::create_dir_all(&base_dir).context("failed to create Talon RPC directory")?;
    }

    let request_path = base_dir.join(REQUEST_FILENAME);
    let response_path = base_dir.join(RESPONSE_FILENAME);
    let state_path = base_dir.join(STATE_FILENAME);

    Ok(TalonPaths {
        request_path,
        response_path,
        state_path,
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
    /// Trigger editing of a previous composer-history entry.
    EditPreviousMessage {
        #[serde(default)]
        steps_back: usize,
    },
    /// Rewind to the latest user message and prefill it for editing.
    EditLastMessage,
    /// Copy the latest visible user request.
    CopyLastRequest,
    /// Copy the latest completed agent response.
    CopyLastResponse,
    /// Generate a fresh title suggestion for the current thread.
    RetitleCurrentSession,
    /// Generate or refresh the leading emoji for the current thread title.
    EmojiCurrentSession,
    /// Interrupt the currently running turn, if any.
    InterruptCurrentTurn,
    /// Exit Codex after graceful shutdown.
    ExitCurrentSession,
    /// Select a model and reasoning effort for future turns.
    SetModel {
        model: String,
        #[serde(default)]
        effort: Option<ReasoningEffort>,
    },
    /// Restart Codex only when no turn is currently running.
    ReloadCurrentSessionIfIdle,
    /// Restart Codex and resume the current session.
    ReloadCurrentSession,
    /// Navigate to the previous entry in the composer history.
    HistoryPrevious,
    /// Navigate to the next entry in the composer history.
    HistoryNext,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TalonResponseStatus {
    Ok,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TalonEditorState {
    pub buffer: String,
    pub cursor: usize,
    pub is_task_running: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_user_request: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recent_user_requests: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recent_agent_responses: Vec<String>,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffort>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TalonModelState {
    pub model: String,
    pub display_name: String,
    pub default_reasoning_effort: ReasoningEffort,
    pub supported_reasoning_efforts: Vec<ReasoningEffort>,
    pub show_in_picker: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TalonAmbientState {
    pub version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub is_task_running: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_user_request: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recent_user_requests: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recent_agent_responses: Vec<String>,
    pub current_model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_reasoning_effort: Option<ReasoningEffort>,
    pub models: Vec<TalonModelState>,
    pub timestamp_ms: u128,
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

pub(crate) fn write_state(paths: &TalonPaths, state: &TalonAmbientState) -> Result<()> {
    let payload = serde_json::to_vec_pretty(state).context("failed to serialize Talon state")?;
    fs::write(&paths.state_path, payload).with_context(|| {
        format!(
            "failed to write Talon state to {}",
            paths.state_path.display()
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
