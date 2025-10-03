use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Parser)]
#[command(about = "Simulate Codex's Talon RPC mutations for testing", version)]
struct Cli {
    /// Initial state JSON file (defaults to empty buffer if omitted)
    #[arg(long)]
    state: Option<PathBuf>,

    /// Request JSON file containing commands
    #[arg(long)]
    request: PathBuf,

    /// Optional path to write the response JSON (defaults to stdout)
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct TalonRequest {
    #[serde(default)]
    commands: Vec<TalonCommand>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum TalonCommand {
    SetBuffer {
        text: String,
        #[serde(default)]
        cursor: Option<usize>,
    },
    SetCursor {
        cursor: usize,
    },
    GetState,
    Notify {
        message: String,
    },
    EditPreviousMessage {
        #[serde(default)]
        steps_back: usize,
    },
    HistoryPrevious,
    HistoryNext,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum TalonResponseStatus {
    Ok,
    NoRequest,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TalonEditorState {
    #[serde(default)]
    buffer: String,
    #[serde(default)]
    cursor: usize,
    #[serde(default)]
    is_task_running: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    task_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cwd: Option<String>,
}

impl Default for TalonEditorState {
    fn default() -> Self {
        Self {
            buffer: String::new(),
            cursor: 0,
            is_task_running: false,
            task_summary: None,
            session_id: None,
            cwd: None,
        }
    }
}

#[derive(Debug, Serialize)]
struct TalonResponse {
    version: u32,
    status: TalonResponseStatus,
    state: TalonEditorState,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    applied: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    timestamp_ms: u128,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut state: TalonEditorState = if let Some(path) = cli.state {
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read state file {}", path.display()))?;
        let mut parsed: TalonEditorState = serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse state JSON from {}", path.display()))?;
        clamp_cursor(&mut parsed);
        parsed
    } else {
        TalonEditorState::default()
    };

    let request_raw = fs::read_to_string(&cli.request)
        .with_context(|| format!("failed to read request file {}", cli.request.display()))?;
    let request: TalonRequest = serde_json::from_str(&request_raw).with_context(|| {
        format!(
            "failed to parse request JSON from {}",
            cli.request.display()
        )
    })?;

    let mut applied = Vec::new();
    let error: Option<String> = None;

    if request.commands.is_empty() {
        // Nothing to do; fall through to response with NoRequest status.
    } else {
        for command in request.commands {
            match command {
                TalonCommand::SetBuffer { text, cursor } => {
                    state.buffer = text;
                    let desired = cursor.unwrap_or_else(|| state.buffer.len());
                    state.cursor = desired.min(state.buffer.len());
                    applied.push("set_buffer".to_string());
                }
                TalonCommand::SetCursor { cursor } => {
                    state.cursor = cursor.min(state.buffer.len());
                    applied.push("set_cursor".to_string());
                }
                TalonCommand::GetState => {
                    applied.push("get_state".to_string());
                }
                TalonCommand::Notify { message } => {
                    let _ = message;
                    // No state change; record applied label for parity with the real TUI.
                    applied.push("notify".to_string());
                }
                TalonCommand::EditPreviousMessage { steps_back } => {
                    let _ = steps_back;
                    applied.push("edit_previous_message".to_string());
                }
                TalonCommand::HistoryPrevious => {
                    applied.push("history_previous".to_string());
                }
                TalonCommand::HistoryNext => {
                    applied.push("history_next".to_string());
                }
            }
        }
    }

    let status = if error.is_some() {
        TalonResponseStatus::Error
    } else if applied.is_empty() {
        TalonResponseStatus::NoRequest
    } else {
        TalonResponseStatus::Ok
    };

    let response = TalonResponse {
        version: 1,
        status,
        state,
        applied,
        error,
        timestamp_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default(),
    };

    let json = serde_json::to_string_pretty(&response)?;

    if let Some(path) = cli.output {
        fs::write(&path, json)
            .with_context(|| format!("failed to write response to {}", path.display()))?;
    } else {
        println!("{}", json);
    }

    Ok(())
}

fn clamp_cursor(state: &mut TalonEditorState) {
    state.cursor = state.cursor.min(state.buffer.len());
}
