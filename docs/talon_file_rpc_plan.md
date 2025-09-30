# Talon File RPC Integration Plan

1. Inspect the composer/input handling to determine how to capture and update buffer/cursor state.
2. Design and implement a Talon file RPC helper (request/response files, JSON schema, async integration).
3. Expose a new keyboard shortcut to trigger the RPC and apply the response in the chat composer (add targeted tests).
4. Provide a simple CLI (`cargo run -p codex-tui --bin talon-send …`) to stage `set_buffer` / `set_cursor` commands under `~/.codex-talon/` and inspect `response.json` (`state` subcommand) for manual invocation.

Status: completed for all steps.
