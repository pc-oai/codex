use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::build_turns_from_rollout_items;
use codex_protocol::protocol::RolloutLine;

#[derive(Clone, Debug)]
pub(crate) struct BrowserData {
    pub(crate) rollout_path: PathBuf,
    pub(crate) nodes: Vec<TreeNode>,
}

#[derive(Clone, Debug)]
pub(crate) struct TreeNode {
    pub(crate) depth: usize,
    pub(crate) label: String,
    pub(crate) detail: String,
}

pub(crate) fn load_browser_data(path: &Path) -> Result<BrowserData> {
    let file =
        File::open(path).with_context(|| format!("failed to open rollout {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut items = Vec::new();
    for line in reader.lines() {
        let line = line.context("failed to read rollout line")?;
        if line.trim().is_empty() {
            continue;
        }
        let Ok(parsed) = serde_json::from_str::<RolloutLine>(&line) else {
            continue;
        };
        items.push(parsed.item);
    }

    let turns = build_turns_from_rollout_items(&items);
    Ok(BrowserData {
        rollout_path: path.to_path_buf(),
        nodes: build_nodes(&turns),
    })
}

fn build_nodes(turns: &[Turn]) -> Vec<TreeNode> {
    let mut nodes = Vec::new();
    for (turn_index, turn) in turns.iter().enumerate() {
        nodes.push(TreeNode {
            depth: 0,
            label: format!(
                "Turn {} [{}]{}",
                turn_index + 1,
                format_turn_status(turn),
                turn.duration_ms
                    .map(|duration| format!(" {duration}ms"))
                    .unwrap_or_default()
            ),
            detail: summarize_turn(turn),
        });
        for item in &turn.items {
            nodes.extend(nodes_for_item(item));
        }
    }
    if nodes.is_empty() {
        nodes.push(TreeNode {
            depth: 0,
            label: "(no replayable transcript items)".to_string(),
            detail: "No structured turn history was reconstructed from this rollout.".to_string(),
        });
    }
    nodes
}

fn nodes_for_item(item: &ThreadItem) -> Vec<TreeNode> {
    match item {
        ThreadItem::UserMessage { content, .. } => vec![leaf(
            1,
            format!("User: {}", summarize_user_inputs(content)),
            summarize_user_inputs(content),
        )],
        ThreadItem::AgentMessage { text, .. } => {
            vec![leaf(
                1,
                format!("Assistant: {}", preview(text)),
                text.clone(),
            )]
        }
        ThreadItem::Reasoning {
            summary, content, ..
        } => vec![leaf(
            1,
            format!("Reasoning: {}", preview(&summary.join(" "))),
            format!(
                "Summary:\n{}\n\nRaw content:\n{}",
                summary.join("\n"),
                content.join("\n")
            ),
        )],
        ThreadItem::Plan { text, .. } => {
            vec![leaf(1, format!("Plan: {}", preview(text)), text.clone())]
        }
        ThreadItem::CommandExecution {
            command,
            aggregated_output,
            status,
            exit_code,
            duration_ms,
            ..
        } => {
            let mut nodes = vec![leaf(
                1,
                format!("Command [{status:?}]: {}", preview(command)),
                format!(
                    "Command:\n{command}\n\nStatus: {status:?}\nExit: {}\nDuration: {}",
                    exit_code
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "n/a".to_string()),
                    duration_ms
                        .map(|value| format!("{value}ms"))
                        .unwrap_or_else(|| "n/a".to_string())
                ),
            )];
            if let Some(output) = aggregated_output {
                nodes.push(leaf(
                    2,
                    format!("Output: {}", preview(output)),
                    output.clone(),
                ));
            }
            nodes
        }
        ThreadItem::McpToolCall {
            server,
            tool,
            result,
            error,
            status,
            ..
        } => {
            let mut nodes = vec![leaf(
                1,
                format!("MCP [{status:?}]: {server}.{tool}"),
                format!(
                    "Server: {server}\nTool: {tool}\nStatus: {status:?}\nError: {}",
                    error
                        .as_ref()
                        .map(|value| value.message.as_str())
                        .unwrap_or("n/a")
                ),
            )];
            if let Some(result) = result {
                nodes.push(leaf(
                    2,
                    "Result".to_string(),
                    serde_json::to_string_pretty(result).unwrap_or_else(|_| format!("{result:?}")),
                ));
            }
            nodes
        }
        ThreadItem::FileChange {
            changes, status, ..
        } => vec![leaf(
            1,
            format!("Files [{status:?}]: {} change(s)", changes.len()),
            format!("{changes:#?}"),
        )],
        ThreadItem::WebSearch { query, action, .. } => vec![leaf(
            1,
            format!("Web search: {}", preview(query)),
            format!("Query: {query}\nAction: {action:?}"),
        )],
        ThreadItem::ImageView { path, .. } => vec![leaf(
            1,
            format!("View image: {}", path.display()),
            path.display().to_string(),
        )],
        ThreadItem::ImageGeneration {
            revised_prompt,
            saved_path,
            ..
        } => vec![leaf(
            1,
            format!(
                "Image generation: {}",
                preview(revised_prompt.as_deref().unwrap_or("(no revised prompt)"))
            ),
            format!(
                "Prompt: {}\nSaved path: {}",
                revised_prompt.as_deref().unwrap_or("(none)"),
                saved_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "(none)".to_string())
            ),
        )],
        ThreadItem::HookPrompt { fragments, .. } => vec![leaf(
            1,
            format!("Hook prompt: {} fragment(s)", fragments.len()),
            fragments
                .iter()
                .map(|fragment| fragment.text.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        )],
        ThreadItem::DynamicToolCall { tool, status, .. } => vec![leaf(
            1,
            format!("Dynamic tool [{status:?}]: {tool}"),
            format!("{item:#?}"),
        )],
        ThreadItem::CollabAgentToolCall { tool, status, .. } => vec![leaf(
            1,
            format!("Agent tool [{status:?}]: {tool:?}"),
            format!("{item:#?}"),
        )],
        ThreadItem::EnteredReviewMode { review, .. } => vec![leaf(
            1,
            "Entered review mode".to_string(),
            format!("{review:#?}"),
        )],
        ThreadItem::ExitedReviewMode { review, .. } => vec![leaf(
            1,
            "Exited review mode".to_string(),
            format!("{review:#?}"),
        )],
        ThreadItem::ContextCompaction { .. } => vec![leaf(
            1,
            "Context compaction".to_string(),
            format!("{item:#?}"),
        )],
    }
}

fn leaf(depth: usize, label: String, detail: String) -> TreeNode {
    TreeNode {
        depth,
        label,
        detail,
    }
}

fn summarize_turn(turn: &Turn) -> String {
    format!(
        "Turn {}\nStatus: {:?}\nItems: {}\nDuration: {}",
        turn.id,
        turn.status,
        turn.items.len(),
        turn.duration_ms
            .map(|value| format!("{value}ms"))
            .unwrap_or_else(|| "n/a".to_string())
    )
}

fn format_turn_status(turn: &Turn) -> &'static str {
    match turn.status {
        codex_app_server_protocol::TurnStatus::InProgress => "running",
        codex_app_server_protocol::TurnStatus::Completed => "done",
        codex_app_server_protocol::TurnStatus::Interrupted => "interrupted",
        codex_app_server_protocol::TurnStatus::Failed => "failed",
    }
}

fn summarize_user_inputs(content: &[codex_app_server_protocol::UserInput]) -> String {
    let text = content
        .iter()
        .filter_map(|item| match item {
            codex_app_server_protocol::UserInput::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ");
    if text.is_empty() {
        "(non-text user input)".to_string()
    } else {
        preview(&text)
    }
}

pub(crate) fn preview(text: &str) -> String {
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    const MAX_CHARS: usize = 72;
    let mut chars = one_line.chars();
    let preview = chars.by_ref().take(MAX_CHARS).collect::<String>();
    if chars.next().is_some() {
        format!("{preview}...")
    } else if preview.is_empty() {
        "(empty)".to_string()
    } else {
        preview
    }
}
