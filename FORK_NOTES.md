# Local fork notes

This is a running note of intentional local divergences from upstream Codex. Keep the reason with the behavior; the reason is usually what gets lost during a rebase.

## Session recognition and resume picker

- Custom-titled sessions are treated as first-class landmarks: their rows are starred and bold, and `Ctrl-T` filters to custom titles only.
  - Why: hand-written titles are much stronger retrieval cues than generated previews.
- The resume table shows a user-message count, preserves leading emoji in titles, and shortens cwd display to the directory basename unless two distinct paths share that basename. Expanded paths replace the home prefix with `~`.
  - Why: message count is a cheap signal of session weight, and compact cwd labels make the table scan faster without losing disambiguation.
- `Ctrl-A` toggles all-directories scope, with the all-directories page warmed in the background at picker startup. `Ctrl-Q` and `Ctrl-X` also exit the picker.
  - Why: all-directories is common enough to deserve one key, but not common enough to justify a visible wait when toggled.
- Thread metadata stores `user_message_count` and backfills historical counts asynchronously.
  - Why: the picker should get a useful count cheaply at read time instead of reparsing every rollout on demand.

## Thread titles and visual identity

- `/retitle` asks for a concise title from the whole conversation. `/emoji` asks for one to three leading emoji, or a full emoji-prefixed title when no meaningful title exists yet.
- Those suggestions run out-of-band in hidden ephemeral forks and only write metadata back to the parent thread.
  - Why: the model needs the full conversation to name it well, but title-generation prompts and answers should not pollute the real transcript.
- Leading title emoji are treated specially in the terminal title: they are surfaced near the front while the textual thread title is shown without duplicating that prefix.
  - Why: emoji are useful glanceable handles when tabs and session lists are crowded.

## Session lifecycle

- Resume, reload, and fork all prefer the saved session cwd automatically when it still exists; if it no longer exists, they fall back to the current cwd.
  - Why: a resumed or branched session should reopen in the project it actually belongs to, without asking a repetitive question when the answer is obvious.
- Interactive `thread/fork` branches from the last completed turn when the source thread is mid-turn, dropping the unfinished final turn instead of inheriting partial-turn context.
  - Why: a fork should start from a stable conversational boundary. Carrying a half-finished user turn into the child is confusing for both the user and the new agent.
- `/reload` re-execs the current session and uses the same saved-cwd rule.
  - Why: local rebuild testing should return to the same working context rather than silently drifting with the shell cwd.

## Editing and small workflow commands

- `Alt-Up` / `Ctrl-E` open a reversible edit-last-message preview when idle. `Esc` cancels without touching history; `Enter` commits the rollback and resubmission.
  - Why: this compresses the old `Esc`, `Esc`, `Enter` path without making the first keypress destructive.
- `Ctrl-X` clears only the composer input.
  - Why: it gives voice and keyboard workflows a safe “clear the draft” action without borrowing `Ctrl-C`, whose meaning changes with app state.
- Local utility slash commands include `/id`, `/copy-last-request`, `/delete`, `/effort`, and `/reload`.
  - Why: they remove small but repeated bits of ceremony from local dogfooding.

## Local-runtime conveniences

- Source builds show a compact local build label in the status line.
  - Why: it should be obvious when a terminal is running the forked debug binary rather than an installed release binary.
- CLI runtime toggles include `--no-mcp-servers`, `--no-project-docs`, `--config-file`, and `--private` / `-P`.
  - Why: wrappers and aliases should be able to shape one process invocation without mutating durable user config, and quick test chats should not have to pollute session history.
- The Talon file-RPC bridge exposes session/editor state and supports small control actions such as edit-last-message, reload, model selection, and copying the last request.
  - Why: voice tooling and launch wrappers need a narrow automation seam, but the Codex TUI should remain the source of truth.

## Startup and observability

- Fresh TUI startup uses the local bundled model catalog immediately, then refreshes `model/list` in the background and updates the UI when the full catalog arrives.
  - Why: startup should become interactive quickly without permanently trading away the newer model list.
- Runtime metrics keep a local in-memory reader alive whenever `runtime_metrics` is enabled, even when no remote exporter is configured.
  - Why: local debug builds should still be able to show timing information; visibility for the operator should not depend on telemetry export.

## Maintenance reminders

- Prefer adding new intentional divergences here when they are introduced, especially if the upstream behavior is plausible enough that a future rebase could accidentally restore it.
- When fork semantics change, keep the pair together: saved-cwd reuse and last-completed-turn branching are part of the same “fork into a coherent workspace” policy.
