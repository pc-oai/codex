# Codex fork session history: 2026-04-23 through 2026-05-06

This is the preceding raw-session-history window for Codex sessions whose
recorded session cwd is exactly:

```text
/Users/pc/code/codex/codex-rs
```

The date window is the two weeks immediately before the May 7 through May 21
audit. The primary evidence is `~/.codex/sessions/2026/{04,05}/*/rollout-*.jsonl`,
filtered by `session_meta.payload.cwd`. Existing memory summaries and the
current fork docs were used as pointers and cross-checks, not as replacements
for the raw logs.

Sessions are grouped by the day in their raw session path. Several May 4 and
May 5 logs continue into later local calendar days; their later turns stay with
the raw file that contains them.

Two read-only subagent passes covered the sparse April-to-May-4 segment and the
dense May 5-to-May-6 segment. The main pass rechecked raw file counts and the
current `FORK_SPEC.md` coverage before writing this summary.

## Inventory

| Raw-session day | Matching files | Accounting note |
| --- | ---: | --- |
| 2026-04-23 | 0 | No exact-cwd `codex-rs` raw session. |
| 2026-04-24 | 0 | No exact-cwd `codex-rs` raw session. |
| 2026-04-25 | 0 | No exact-cwd `codex-rs` raw session. |
| 2026-04-26 | 0 | No exact-cwd `codex-rs` raw session. |
| 2026-04-27 | 0 | No exact-cwd `codex-rs` raw session. |
| 2026-04-28 | 0 | No exact-cwd `codex-rs` raw session. |
| 2026-04-29 | 0 | No exact-cwd `codex-rs` raw session. |
| 2026-04-30 | 0 | No exact-cwd `codex-rs` raw session. |
| 2026-05-01 | 0 | No exact-cwd `codex-rs` raw session. |
| 2026-05-02 | 0 | No exact-cwd `codex-rs` raw session. |
| 2026-05-03 | 0 | No raw session directory for this day. |
| 2026-05-04 | 6 | Startup/runtime work, long Talon/reload automation, and short helper logs. |
| 2026-05-05 | 23 | One large session-workflow thread, restored startup/history follow-up, many stubs and unrelated quick tasks. |
| 2026-05-06 | 4 | Fork-notes continuation, title-worker bug fix, and deploy-key checks launched from this cwd. |

One May 6 raw file includes its own metadata plus a resumed metadata record for
the long May 5 session-workflow thread.

## What landed in the fork

| Theme | Implemented behavior and reason |
| --- | --- |
| Startup and local config | Startup work moved bundled-model and first-thread work off the critical input path, restored background model refresh, reduced startup layout jitter, and split fork-only config behind `--config-file`. The driver was faster local startup without breaking production Codex config. |
| Local runtime visibility | Runtime metrics became visible locally even when no remote metrics exporter is available. The TUI timing path should help a debug build operator, not require remote telemetry plumbing. |
| Private and scoped runtime flags | Interactive no-history mode gained `--private` / `-P`, and later the one-home cleanup kept flags such as no-MCP and no-project-docs instead of carrying split Codex homes. |
| Session retrieval cues | The resume picker gained custom-title emphasis and filtering, current-directory/all-directory switching, warmed broad queries, user-message counts and backfill, compact cwd labels, emoji/title landmarks, and local launcher corrections. The repeated goal was making long-lived session lists scannable. |
| Titles and naming | `/retitle` and `/emoji` grew hidden ephemeral-fork workers that name the parent thread without polluting its visible transcript. The later May 6 bug fix preserved streamed child answers when the completion envelope is empty. |
| Reversible edits | Edit-last-message evolved from immediate rollback into `Ctrl-E` preview state with cancel/commit behavior, visible footer cues, repeat-back navigation, and later interrupt/edit follow-up ideas. Faster prompt correction should not erase history just because a shortcut was explored. |
| Session lifecycle | Resume, reload, and fork reuse saved cwd where possible; in-progress fork work branches from stable completed context; local slash helpers such as `/reload`, `/delete`, `/id`, `/effort`, and copy helpers reduce repeated local ceremony. |
| Automation and reload | Terminal titles and Talon/thread-ID surfaces exposed session identity; reload draft handoff and command/voice work kept local Codex controllable during local rebuild loops. |
| Keyboard and composer workflow | `Ctrl-X` clear/discard and queued-input steering work started here, together with a broader local keymap path that later fed the fork contract. |

## Ideas not finished or deliberately deferred

- The first startup-performance pass did not finish a single-pass config-load
  refactor or fully defer logging/OTel/state setup. It chose the safer startup
  cuts first.
- Session color metadata for eventual Ghostty tab-color sync was discussed in
  the resume-picker thread and deferred.
- Automatic hidden title generation after the first turn was discussed. The
  guardrail was clear: never overwrite a title the user chose. The feature was
  not landed in this window.
- Direct in-place editing of old user or assistant history was discussed and
  rejected as a default direction because it can make visible history disagree
  with model context. Rewind/branch semantics stayed the honest path.
- The long Talon/reload automation thread discussed richer state-file surfaces,
  last active working directory, subprocess PID visibility, automatic stale
  version reloads, and agent-exposed self-retitling. Those were not all fork
  code in this window.
- The exact-cwd deploy-key and Datadog quick tasks in May 5-6 are cwd noise for
  this fork audit. They belong in the raw inventory but do not define fork
  behavior.

## Day by day

### 2026-04-23 through 2026-05-03

No raw file under these dated session folders recorded the exact `codex-rs` cwd
used for this audit. The first matching raw session in this earlier window is
on May 4.

### 2026-05-04

Raw files:

- `rollout-2026-05-04T11-42-31-019df44c-8f87-7723-a512-eb58e30c2ab1.jsonl`
- `rollout-2026-05-04T20-31-22-019df630-bd35-70a3-a8db-3a001e1400fb.jsonl`
- `rollout-2026-05-04T20-49-55-019df641-ba47-7ac2-a8a6-839e3f676bbd.jsonl`
- `rollout-2026-05-04T21-15-51-019df659-76bc-7832-be99-d095ac161c90.jsonl`
- `rollout-2026-05-04T21-31-19-019df667-a047-7d61-b0b3-e3c1ce6a0970.jsonl`
- `rollout-2026-05-04T21-32-42-019df668-e550-78b0-b053-88955a456e2b.jsonl`

The startup/runtime cluster is mainly the `20-31-22` and `20-49-55` pair.

- Implemented or validated:
  - Startup profiling targeted time until the composer accepts text, not only
    `--help` time.
  - Fresh-session startup stopped waiting on several first-paint blockers,
    including first thread startup and startup skills/model work.
  - Bundled model data was used for immediate startup while the full model list
    refresh remains deferred in the background.
  - Startup blank-line jitter was investigated and cleaned up.
  - The local fork moved local-only config toward an explicit `--config-file`
    path so production Codex is not broken by fork-only keys.
  - Runtime metrics got a local in-memory reader path when enabled without an
    exporter.
  - Interactive private/no-history mode became `--private` / `-P`.
- Motivation and reasoning:
  - The user cared about "can type now" latency.
  - Deferred work must still happen later; faster startup was not permission to
    lose the internal model catalog forever.
  - Local debug timing should not disappear merely because a Statsig exporter
    is disabled in debug builds.
  - Fork-only config should not make the installed production binary reject the
    main config file.
- Deferred:
  - A deeper single-pass config load and broader startup initialization split
    remained future work.

The long `11-42-31` log starts as session-ID/title automation and keeps
accumulating local Codex automation turns after May 4.

- Implemented or carried forward:
  - Terminal titles expose a raw session/thread ID surface.
  - Talon and later SpeechLab-facing paths use that identity for narrow local
    control and copying.
  - Reload draft handoff work preserves visible composer text during local
    reload flows.
  - Slash/helper surfaces and later menu/voice work grew around reload, ID,
    copy-last-request/response, retitle, emoji, and effort shortcuts.
- Motivation:
  - Local rebuilds and voice control need a stable session handle and should not
    drop the draft being dictated.
- Ideas outside the durable fork core:
  - richer state-file contents, subprocess visibility, stop-hook reload ideas,
    and self-retitle tools were explored later in this same raw file.

The other May 4 files are cwd matches but not the core Codex feature thread:

- `21-15-51` moved into SpeechLab brightness-command work after launch.
- `21-31-19` is a read-only CPU/build investigation: the observed load during
  a Codex Rust build came from many `rustc` jobs rather than the suspected
  CrowdStrike process, and lighter placement for one config parsing test was
  discussed but not patched.
- `21-32-42` is metadata-only.

### 2026-05-05

This is the noisy raw-session day. Twenty-three files match the cwd, but many
are one-line session metadata stubs or unrelated quick tasks from that
directory. The raw appendix lists every one.

The large product thread is:

- `rollout-2026-05-05T12-10-55-019df98c-eb13-7963-8f32-1f1ae9c6cc6c.jsonl`

It continues into May 6 turns and forms the first broad local session-workflow
bundle.

- Implemented:
  - Custom-titled resume rows became stronger landmarks and `Ctrl-T` filters to
    titled sessions.
  - Picker `Ctrl-A` toggles current cwd versus all directories, with broad query
    warmup to avoid a visible toggle delay.
  - Message count metadata and historical backfill were added as a cheap
    session-weight signal.
  - Cwd labels shrink to basename unless distinct paths collide, and home paths
    shrink to `~` when expanded.
  - Emoji/title experiments fed terminal-title and session-list recognition.
  - `/retitle` and `/emoji` use conversation-aware title workers, later moved
    out of the visible main transcript through hidden ephemeral forks.
  - Resume, reload, and fork reuse saved cwd where it still exists.
  - Forking an in-progress thread was shaped around stable completed-turn
    boundaries instead of carrying confusing partial-turn context.
  - `/delete`, `/reload`, `/id`, copy helpers, and other local slash helpers
    were added or refined.
  - Edit-last-message grew direct shortcut work that leads into the reversible
    `Ctrl-E` preview model.
  - Previous-turn timing remains visible in muted form while the next turn is
    running.
- Motivation and reasoning:
  - Session finding was becoming a real local workflow cost.
  - Session metadata should make lists cheap to scan without reparsing every
    rollout synchronously.
  - Title and emoji generation should see the whole conversation while leaving
    the real thread transcript clean.
  - Prompt correction needs a shortcut, but an exploratory shortcut should not
    immediately orphan the old assistant response.
- Discussed or not pursued:
  - Session color storage for later Ghostty tab sync.
  - Automatic asynchronous title generation after first turn for still-untitled
    threads.
  - Editing old assistant/user history "in place" as a different product
    semantic from rewind and rerun.

The `07-32-31` file is a restored-session/history follow-up to the
startup/private-mode work. It checks the printed resume ID against the actual
resumable session and continues the careful state/history cleanup context.

The non-product or helper files include:

- translation checks in `08-35-54` and `08-36-14`,
- Datadog/dashboard checks in `08-56-56` and `08-59-57`,
- a hidden title worker in `14-03-33`,
- many one-line metadata files around `08:39` through `08:55` and `13:01`
  through `13:43`.

They count for the exact-cwd inventory but do not add fork contract surface by
themselves.

### 2026-05-06

Raw files:

- `rollout-2026-05-06T11-14-39-019dfe7f-c670-71b3-a1c8-ab8fac1bbd40.jsonl`
- `rollout-2026-05-06T13-36-44-019dff01-d7fd-7e40-b695-be9f6ff28549.jsonl`
- `rollout-2026-05-06T17-10-34-019dffc5-9d3a-7892-9427-2f8cc46e09e5.jsonl`
- `rollout-2026-05-06T17-11-18-019dffc6-4ba8-7ac1-b049-d52c8031b79c.jsonl`

The `11-14-39` file continues the session-workflow work and explicitly starts
fork documentation.

- Implemented or recorded:
  - The repo-root `FORK_NOTES.md` was created and expanded from actual local
    history so future rebases preserve rationale, not only source hunks.
  - The session-workflow bundle was committed as
    `615a82f60acc` (`tui: expand local session workflows`).
  - The documented bundle includes resume picker cues, title/emoji flows,
    saved cwd behavior, reversible edit preview, local slash helpers, and local
    timing/footer behavior.
- Motivation:
  - These are exactly the kind of personal fork behaviors that can disappear
    during an upstream rebase if only the current diff is remembered.

The `13-36-44` file fixes a title-worker integration bug.

- Implemented:
  - `/retitle` and `/emoji` remember the hidden child worker's streamed final
    assistant message from item notifications.
  - Completion then consumes that remembered text even when later
    `TurnCompleted` has `items: []`.
- Reasoning:
  - The app-server empty completion envelope is a valid event shape, so the
    title worker had to follow the real event order instead of waiting for a
    populated final envelope.
- Validation caveat:
  - Focused title-worker tests passed while broader TUI suite failures elsewhere
    in the dirty tree were left alone.

The `17-10-34` and `17-11-18` files are deploy-key experiments launched from
this cwd. They prove GitHub key remove/re-add and clipboard behavior, not a
Codex fork feature.

## Spec reconciliation

The current `FORK_SPEC.md` already covers the durable fork behaviors from this
earlier window:

- startup with bundled models plus background refresh,
- runtime metrics without exporter coupling,
- private and invocation-local config/runtime switches,
- `Ctrl-X`, custom-title picker behavior, cwd/message-count cues, saved cwd,
- `/retitle`, `/emoji`, hidden title workers, empty completion handling,
- edit preview and local slash helper surfaces,
- Talon/local automation and reload draft handoff.

I did not find a new durable earlier-window behavior that needs another
`FORK_SPEC.md` edit beyond the spec work already in progress from the later
window.

## Appendix: raw matching session files

### 2026-05-04

- `/Users/pc/.codex/sessions/2026/05/04/rollout-2026-05-04T11-42-31-019df44c-8f87-7723-a512-eb58e30c2ab1.jsonl`
- `/Users/pc/.codex/sessions/2026/05/04/rollout-2026-05-04T20-31-22-019df630-bd35-70a3-a8db-3a001e1400fb.jsonl`
- `/Users/pc/.codex/sessions/2026/05/04/rollout-2026-05-04T20-49-55-019df641-ba47-7ac2-a8a6-839e3f676bbd.jsonl`
- `/Users/pc/.codex/sessions/2026/05/04/rollout-2026-05-04T21-15-51-019df659-76bc-7832-be99-d095ac161c90.jsonl`
- `/Users/pc/.codex/sessions/2026/05/04/rollout-2026-05-04T21-31-19-019df667-a047-7d61-b0b3-e3c1ce6a0970.jsonl`
- `/Users/pc/.codex/sessions/2026/05/04/rollout-2026-05-04T21-32-42-019df668-e550-78b0-b053-88955a456e2b.jsonl`

### 2026-05-05

- `/Users/pc/.codex/sessions/2026/05/05/rollout-2026-05-05T07-32-31-019df88e-0b12-7ad2-b354-445fbfd81b63.jsonl`
- `/Users/pc/.codex/sessions/2026/05/05/rollout-2026-05-05T08-35-54-019df8c8-12ed-77d1-90a2-da581f5dde08.jsonl`
- `/Users/pc/.codex/sessions/2026/05/05/rollout-2026-05-05T08-36-14-019df8c8-5fc5-7840-8d21-5b1a611c2352.jsonl`
- `/Users/pc/.codex/sessions/2026/05/05/rollout-2026-05-05T08-39-57-019df8cb-c4d5-7861-aede-4b7c4e87b475.jsonl`
- `/Users/pc/.codex/sessions/2026/05/05/rollout-2026-05-05T08-40-10-019df8cb-f9ef-7811-8580-c1709fe4d2b7.jsonl`
- `/Users/pc/.codex/sessions/2026/05/05/rollout-2026-05-05T08-40-50-019df8cc-9426-71b1-b87e-ac61a3509ed0.jsonl`
- `/Users/pc/.codex/sessions/2026/05/05/rollout-2026-05-05T08-40-51-019df8cc-992e-7252-a89d-af22ab0a692c.jsonl`
- `/Users/pc/.codex/sessions/2026/05/05/rollout-2026-05-05T08-45-39-019df8d0-fd8b-7c43-b572-4102af45cfbe.jsonl`
- `/Users/pc/.codex/sessions/2026/05/05/rollout-2026-05-05T08-45-47-019df8d1-1c52-7713-ba27-fe7369db53ae.jsonl`
- `/Users/pc/.codex/sessions/2026/05/05/rollout-2026-05-05T08-48-05-019df8d3-3a99-78e2-b343-7d23605770d1.jsonl`
- `/Users/pc/.codex/sessions/2026/05/05/rollout-2026-05-05T08-52-49-019df8d7-8d91-7bf3-bd83-74fc41a18787.jsonl`
- `/Users/pc/.codex/sessions/2026/05/05/rollout-2026-05-05T08-53-32-019df8d8-3600-7a92-9b83-99aea6f0dec8.jsonl`
- `/Users/pc/.codex/sessions/2026/05/05/rollout-2026-05-05T08-55-27-019df8d9-f8d5-7862-a665-ed00d1197d7c.jsonl`
- `/Users/pc/.codex/sessions/2026/05/05/rollout-2026-05-05T08-55-34-019df8da-1104-7002-a1df-da15eded5309.jsonl`
- `/Users/pc/.codex/sessions/2026/05/05/rollout-2026-05-05T08-56-56-019df8db-517e-79a2-88e7-8b9fea7cde00.jsonl`
- `/Users/pc/.codex/sessions/2026/05/05/rollout-2026-05-05T08-59-57-019df8de-1489-7a90-aed7-0481f973f24e.jsonl`
- `/Users/pc/.codex/sessions/2026/05/05/rollout-2026-05-05T12-10-55-019df98c-eb13-7963-8f32-1f1ae9c6cc6c.jsonl`
- `/Users/pc/.codex/sessions/2026/05/05/rollout-2026-05-05T13-01-42-019df9bb-6a8f-7250-8c98-b7baedc17466.jsonl`
- `/Users/pc/.codex/sessions/2026/05/05/rollout-2026-05-05T13-41-53-019df9e0-360f-7ee1-a423-c97e0de506dc.jsonl`
- `/Users/pc/.codex/sessions/2026/05/05/rollout-2026-05-05T13-41-57-019df9e0-4351-7433-96d9-ff886a456bbf.jsonl`
- `/Users/pc/.codex/sessions/2026/05/05/rollout-2026-05-05T13-42-10-019df9e0-7610-77b1-aced-9696ae727417.jsonl`
- `/Users/pc/.codex/sessions/2026/05/05/rollout-2026-05-05T13-43-50-019df9e1-fc6f-7901-9465-bd5e57df0653.jsonl`
- `/Users/pc/.codex/sessions/2026/05/05/rollout-2026-05-05T14-03-33-019df9f4-0ab5-7e10-a36e-3bd659b1760a.jsonl`

### 2026-05-06

- `/Users/pc/.codex/sessions/2026/05/06/rollout-2026-05-06T11-14-39-019dfe7f-c670-71b3-a1c8-ab8fac1bbd40.jsonl`
- `/Users/pc/.codex/sessions/2026/05/06/rollout-2026-05-06T13-36-44-019dff01-d7fd-7e40-b695-be9f6ff28549.jsonl`
- `/Users/pc/.codex/sessions/2026/05/06/rollout-2026-05-06T17-10-34-019dffc5-9d3a-7892-9427-2f8cc46e09e5.jsonl`
- `/Users/pc/.codex/sessions/2026/05/06/rollout-2026-05-06T17-11-18-019dffc6-4ba8-7ac1-b049-d52c8031b79c.jsonl`
