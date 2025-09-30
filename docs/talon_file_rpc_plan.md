# Talon File RPC Integration Plan

1. Inspect the composer/input handling to determine how to capture and update buffer/cursor state.
2. Design and implement a Talon file RPC helper (request/response files, JSON schema, async integration).
3. Expose a new keyboard shortcut to trigger the RPC and apply the response in the chat composer (add targeted tests).
4. Provide a simple CLI (`cargo run -p codex-tui --bin talon-send …`) to stage `set_buffer` / `set_cursor` commands under `~/.codex-talon/` and inspect `response.json` (`state` subcommand) for manual invocation.
5. Add a `notify` command path so Talon/CLI can trigger lightweight Codex toast messages for debugging.

Status: completed for all steps.

## RPC Commands

All requests are written to `~/.codex-talon/request.json` as `{"commands": [ … ]}` and a matching response (with `state`, `applied`, `timestamp_ms`, etc.) appears in `response.json`:

| Command | JSON payload | Effect |
| --- | --- | --- |
| `set_buffer` | `{ "type": "set_buffer", "text": "Hello", "cursor": 5 }` | Replace composer text and optionally reposition the cursor. |
| `set_cursor` | `{ "type": "set_cursor", "cursor": 12 }` | Move cursor to the specified byte offset. |
| `get_state` | `{ "type": "get_state" }` | Return current composer state without modifying anything. |
| `notify` | `{ "type": "notify", "message": "Codex says hi" }` | Emit an inline info message inside Codex. |

Every response includes `state` with `buffer`, `cursor`, `is_task_running`, and `task_summary` (live status header if active). The `applied` array lists each command label (`set_buffer`, `set_cursor`, `get_state`, or `notify`) that was processed.
