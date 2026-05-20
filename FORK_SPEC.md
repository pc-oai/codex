# Local fork specification

This file is the behavioral contract for Phil's local Codex fork. Use it when
an upstream merge or rebase has a large conflict surface and it is not enough
to see that the tree compiles.

`FORK_NOTES.md` is the short running note of why these divergences exist. This
file is the fuller motivation, pre-merge inventory, and post-merge validation
guide.

## Historical baseline

The local tip immediately before the May 20, 2026 merge of `upstream/main` is:

- tag: `pc/local-main-before-upstream-merge-20260520`
- commit: `d9aaee7ac41186e264dea6b72c9cfe7530fb7eb2`
- merge commit that used it as first parent:
  `fa30bac836a3b347922c37200479f07444a82a12`
- upstream parent merged into it:
  `c3faea0b09cadbe74f62503fc7cb3832d82a3ff8`

The fork delta described here is the 33 local commits in:

```sh
git log --reverse --oneline \
  fa30bac836a3b347922c37200479f07444a82a12^2..pc/local-main-before-upstream-merge-20260520
```

Post-merge fixups on `main` after `fa30bac` are not the baseline for this
spec. If the merge is redone, validate the result against the tag first and
then decide which post-merge fixups still apply. Lasting post-merge follow-ups
are listed separately below so this file still accounts for local behavior
added after the anchor was taken.

## Motivation and philosophy

This fork is for local Codex dogfooding in long-running terminal sessions where
Phil keeps many active threads, rebuilds the binary frequently, and drives part
of the UI through voice automation. Upstream defaults can be reasonable for a
general product and still be wrong for that loop. The local changes protect the
parts of that loop that are expensive to lose during a merge: finding the right
thread quickly, staying in the right project context, making edits reversible,
keeping automation narrow and reliable, and seeing enough runtime state to
trust a locally built binary.

The feature list below should be read through these principles:

1. Sessions are working objects, not just transcript files. A thread needs
   retrieval cues, a saved cwd, a lightweight work state, reliable resume
   behavior, and a stable identity across terminal tabs and automation. Names,
   emoji, message counts, short selectors, open-session signals, and
   active/parked/done state exist to reduce search cost without overloading
   archive or delete.
2. Fast actions should stay reversible. The fork shortens repeated keyboard and
   voice-driven workflows, but tries not to turn convenience into silent data
   loss. Edit-last-message previews, draft restoration, queue controls, paste
   visibility, stable fork boundaries, and replayed command output all preserve
   context while making iteration faster.
3. Local automation should extend the TUI, not replace it. Talon, wrappers, and
   per-session sockets need enough state and command surface to be useful, but
   the TUI remains the owner of thread state. Process-local flags are preferred
   when a wrapper or test run should shape one invocation without mutating
   durable config.
4. Local builds need visible evidence. Rebuild testing is only useful when it
   is clear which binary is running and what it is doing. Build labels, archived
   numbered binaries, local runtime metrics, footer progress, and optional
   Ghostty progress exist to make a live debug session inspectable without
   depending on remote telemetry.
5. Responsiveness matters more than ceremony in the local loop. Picker warmup,
   bundled-model bootstrap with background refresh, compact agent switching,
   and session-local utility commands remove waits and repeated prompts where
   the fork already has a strong answer.
6. Experiments should remain identifiable. Transcript browser prototypes,
   condensed scrollback views, and unsafe local model-context overrides are
   useful local tools, but they should stay visible as deliberate local surface
   area rather than being mistaken for inherited upstream behavior.

When a future merge changes implementation shape, preserving these properties
matters more than preserving old code line for line. When it breaks one of these
properties, treat that as a fork regression even if the merged tree compiles.

## Product contract

### Session discovery and identity

- The resume and fork flows accept exact UUIDs, thread names, and unique short
  hexadecimal UUID fragments. Ambiguous short fragments open a picker instead
  of choosing silently.
- The resume picker makes hand-named sessions easy to recover. Custom titles
  stand out, leading emoji in titles survive display, and the picker can filter
  to titled sessions.
- Picker rows carry fast retrieval cues: a user-message count, compact cwd
  labels with disambiguation when basenames collide, a short session-id suffix,
  and an open/closed signal when that state is known.
- All-directory lookup stays one keypress away and is warmed so common picker
  toggles do not feel like a cold fetch.
- Thread metadata persists retrieval state instead of reparsing old rollouts
  on every read. The local fork adds durable `user_message_count` data and
  durable user lifecycle state that travels through state storage, thread
  stores, app-server payloads, and TUI consumers.
- User lifecycle state is separate from destructive or storage lifecycle
  actions: active, parked, and done are saved work states, while archive and
  delete keep their own meanings.

### Titles and terminal identity

- `/retitle` asks for a concise title from the conversation and `/emoji` asks
  for a leading emoji update. These helpers run in hidden ephemeral forks so
  naming prompts and answers do not pollute the real thread transcript.
- Streamed title suggestions must still be applied when valid, including when
  the final turn-completed envelope is empty and the usable suggestion arrived
  earlier as a completed agent-message item. Suggestion requests use the live
  config for the thread, not a stale startup snapshot.
- Terminal titles stay compact and thread-aware. Leading title emoji remain
  useful tab landmarks without duplicating the title text.

### Session lifecycle and editing

- Resume, reload, and fork prefer the saved thread cwd while it still exists.
  They fall back when the saved path is gone instead of forcing an avoidable cwd
  prompt on the usual path.
- Forks branch from a stable completed turn when the source is mid-turn.
- `/reload` and the reload key path restart the current session in a fresh
  process without losing the intended working context.
- Resumed limited-history threads rebuild persisted `exec_command` output into
  command history cells. Reloading a thread must not drop command output that
  still exists in saved response items.
- Composer drafts survive edit and resume handoffs. Edit-last-message can
  temporarily replace a non-empty composer draft, and cancel restores the
  displaced draft. Reload handoff preserves enough state that a rebuild test
  does not throw away active draft work.
- Edit-last-message remains reversible before commit. The idle shortcut opens
  a preview, `Esc` cancels it, and submission performs rollback and resubmit.
  `Ctrl-E` can claim the shortcut when line-end movement would be a no-op, but
  normal editor movement still wins while the cursor is inside a draft.
- App-server turn start supports rollback-driven edit flows, and the TUI routes
  rollback notifications through its thread state instead of losing the edit
  context during resume or side switching.
- Open subagent trees survive app-server resume and TUI reload restart.

### Composer and keyboard workflow

- `Ctrl-X` clears the composer input or discards the relevant queued draft
  without borrowing `Ctrl-C`.
- Main-surface key bindings include `Ctrl-R` to reload the current session and
  resume it in a fresh process, plus condensed transcript toggling. Keymap
  config is forward-compatible at load time so an older local binary ignores
  unknown future action names while generated schema stays strict for
  validation.
- Rename, retitle, queued-message edit, queued-message discard, queued-message
  steer, and reverse-history bindings remain configurable local workflow
  actions.
- Live keymap edits update queued-input footer hints from the newly resolved
  bindings. A picker that changes the edit binding must not leave the queued
  preview advertising an older default.
- Local slash helpers keep repeated dogfooding work close to the session:
  `/id`, `/copy-last-request`, `/delete`, `/effort`, and `/reload`.
- The local default queued-message edit bindings keep both `Ctrl-E` and
  `Alt-E`, not only the upstream arrow-key paths.
- Pasted text stays intact for submission while the inline display threshold is
  configurable with `paste_text_inline_char_limit`.
- Local model experiments can opt in to a context window above the catalog max
  with `model_context_window_allow_unsafe_override`.

### Transcript views and agents

- The local TUI has a condensed message-only transcript mode for terminal
  scrollback.
- The fork includes transcript browser prototypes for outline and tree-style
  exploration of saved conversations. Treat these binaries as local
  experiments unless they are intentionally promoted.
- Agent navigation has a compact menu and a prewarm path so switching among
  active agent threads is quick and visible in the footer/main controls. The
  menu stays as a composer-preserving floating chooser, and `Ctrl-A` can open
  it from the composer when line-start movement has no more work to do.
- `Alt-[` and `Alt-]` rotate to the previous and next agent thread. These
  explicit bracket shortcuts are part of the local workflow and must not be
  replaced by `Alt-Left` / `Alt-Right` word-motion chords.
- Active-agent footer labels refresh when a new subagent thread starts so the
  visible count and working-state marker do not lag behind the picker.

### Local automation and runtime signals

- Source builds go through `scripts/local-build-codex` or `just local-build`
  so the visible local build number advances. Numbered local binaries are
  archived under `target/local-builds/vN/codex` so a known local build can be
  started again directly.
- Local CLI/runtime overrides remain available for dogfooding wrappers,
  including config-file selection, no-MCP, no-project-docs, private mode, and
  loader override propagation into the standalone exec binary.
- Fresh local TUI startup uses bundled model data for the first in-process
  render instead of waiting on `model/list`. It refreshes models from the app
  server in the background and updates model-dependent UI after that result
  arrives.
- Fresh local TUI startup and clear-screen redraws do not print the boxed
  `OpenAI Codex` session header into terminal history. The prompt and compact
  footer/status surfaces are enough; the large model/directory/permissions
  banner is startup noise in the local loop.
- The Talon integration is per session. Keep the local file-RPC bridge and the
  session-owned command sockets working together: they expose editor/session
  state and narrow control actions through `talon_send`/`talon_sim`, ambient
  state, and the live session socket. The control surface includes model
  selection, rename/retitle/emoji operations, edit/reload actions, copying
  recent requests or responses, interruption, and local build number
  visibility.
- Talon setup must retry once a newly created chat widget finally has a thread
  ID. Starting the widget before the thread ID exists must not leave the
  per-session socket and state files missing for the rest of that session.
- A live Talon session socket is also a useful signal for open local sessions
  in the picker.
- The status line has a split local footer surface: configured items stay on
  the left while the local build label and MCP startup progress stay on the
  right when present. MCP startup progress belongs in the footer as compact
  `MCP: done/total` progress instead of taking over the main transcript/status
  header. Compact context-used and timing summaries may also use the right side
  when configured.
- Ghostty gets terminal-native indeterminate progress while a turn runs when
  `tui.terminal_progress_bar` is enabled. The switch defaults off. Progress is
  cleared on completion or widget drop and is handed across thread-widget
  replacement to avoid flicker.
- Runtime metrics stay available locally when runtime metrics are enabled even
  if no remote exporter is configured.

### State compatibility

- The merged state runtime must accept databases that already applied the
  fork's old thread-metadata migrations before upstream reused those migration
  numbers. It remaps the old local applied-migration records forward so startup
  can run the upstream `0030` through `0034` migrations instead of treating the
  database as damaged.

## Validation after an upstream merge

Start by keeping the historical anchor available:

```sh
git show --no-patch pc/local-main-before-upstream-merge-20260520
git log --reverse --oneline \
  fa30bac836a3b347922c37200479f07444a82a12^2..pc/local-main-before-upstream-merge-20260520
```

High-signal static anchors include:

| Behavior | Pre-merge anchors |
| --- | --- |
| Short session selectors | `SessionSelectorLookup`, `lookup_session_selector_with_app_server`, `thread_id_contains_normalized_fragment` |
| Ghostty turn progress | `sync_managed_terminal_progress`, `take_managed_terminal_progress`, `show_terminal_progress_indeterminate` |
| Talon session sockets | `start_socket_acceptor`, `session_command_socket_is_live`, `TalonSocketRequest` |
| Resume/edit handoff | `reload_handoff.rs`, rollback turn start protocol, draft preservation app tests |
| Thread retrieval state | `ThreadUserState`, `user_message_count`, state migrations and thread-store fields |
| Local title helpers | `thread_name_suggestion.rs`, `/retitle`, `/emoji`, live config use |
| Fast model bootstrap | `local_bootstrap_models`, `models_list_with_request_handle`, `ModelsLoaded` |
| Startup header suppression | `clear_ui_header_lines_with_version`, `SessionHeaderHistoryCell` |
| Footer status split | `set_status_line_right`, `mcp_startup_progress_label`, `local_build_label` |
| Compact agent switching | `show_agent_menu`, `AgentMenu`, active-agent footer label refresh |
| Transcript experiments | `ToggleCondensedTranscriptView`, `transcript_outline`, `transcript_tree` |

Then validate behavior rather than just conflict resolution:

1. Check that the local fork surface still has code anchors for Talon session
   sockets and startup retry, thread title suggestions, reload handoff,
   terminal progress, the local build helper and archive path, user lifecycle
   state, old-migration remapping, bundled-model startup with background model
   refresh, suppressed boxed startup history header, short session selectors,
   and the transcript prototype binaries.
2. Regenerate or inspect app-server schema output when thread API payloads move.
   The local delta touches thread delete, rollback start, thread user state, and
   thread metadata payloads.
3. Review TUI snapshots for the resume picker, footer/key hints, title/status
   surfaces including the right-side local build/MCP progress area, queued
   input previews, paste placeholders, condensed transcript, floating agent
   menu, and edit-last-message surfaces.
   Treat tests and snapshots that still name a fork behavior as inventory:
   when a merged test target stops compiling because its implementation anchor
   disappeared, investigate the lost behavior before deleting or weakening the
   test.
4. Exercise the picker manually with a titled thread, an emoji-titled thread,
   a unique short session-id fragment, and an ambiguous short fragment.
5. Exercise edit and reload manually with a non-empty draft so rollback
   preview, cancel restoration, submit, edit-again, resume, and reload handoff
   can be checked for lost composer text.
6. Exercise the Talon socket path against a live TUI session and confirm that
   state reads and at least one command round trip over the session socket.
   Include a fresh-session startup where the widget obtains its thread ID after
   the first Talon setup attempt.
7. In Ghostty, run a real turn with terminal progress enabled and verify the
   terminal progress bar appears only while work is active.
8. Use the local build script for a source build and verify the local build
   marker changes in the running TUI and the numbered archived binary exists.

Useful focused test areas after code changes are:

- `cargo test -p codex-tui`
- `cargo test -p codex-app-server-protocol`
- focused app-server thread tests when thread payloads or state routing move
- focused state/thread-store coverage when migrations or thread metadata move

If a new merge compiles but fails this checklist, prefer treating it as a lost
fork behavior until the difference is explained.

## Commit inventory

These are the pre-merge local commits in history order and the behavior they
feed into this spec.

| Commit | Local change | Spec area |
| --- | --- | --- |
| `7ea001a04639` | Add `Ctrl-X` composer clear | Composer workflow |
| `7101a98f8053` | Restore Talon file RPC support | Local automation |
| `74038d1d3688` | Save local TUI/config work before upstream rebase | Baseline consolidation |
| `b37de3f14760` | Repair rebase fallout and startup behavior | Baseline consolidation |
| `facd3e146d69` | Add local fork notes | Maintenance |
| `26f8024eaea0` | Expand local fork notes | Maintenance |
| `615a82f60acc` | Expand local session workflows, metadata, naming, delete, reload, local flags | Session identity and lifecycle |
| `280f8091903f` | Keep streamed title suggestions | Titles |
| `a8766b83b944` | Preserve composer drafts across edit and resume | Editing |
| `820635042674` | Add unsafe context override and composer/keymap controls | Composer workflow |
| `7ef47d4bc448` | Restore resumed exec output history replay | Session lifecycle |
| `21d96564576e` | Add versioned local build helper | Local runtime |
| `752642e1dd56` | Configure pasted text inline threshold | Composer workflow |
| `73d4cc00e9f6` | Show Ghostty progress during turns | Runtime signals |
| `a80b5d32bd54` | Support rollback turn starts for edit flows | Editing and app-server |
| `f1dcbb148741` | Restore open subagent trees on resume | Agents |
| `8f767d057982` | Restore reload trees across process restart | Session lifecycle |
| `752ad85ac4db` | Add condensed transcript view | Transcript views |
| `7db88571b926` | Refresh composer and session shortcuts | Keyboard workflow |
| `c629b89bb219` | Serve Talon commands over per-session sockets | Local automation |
| `0c2d8d29f01a` | Support short session selectors | Session discovery |
| `5065e5894b1b` | Add agent switcher menu and prewarm | Agents |
| `553fb35368c6` | Compact thread-aware terminal titles | Titles |
| `107bdc559b10` | Refresh queued hint bindings after keymap edits | Coverage |
| `1f08362c8cb7` | Cover resumed history replay | Coverage |
| `0e80674bb199` | Cover side rename and copy remap edges | Coverage |
| `94210679c239` | Refresh generated config schema | Generated artifacts |
| `ad5a44589885` | Refresh source-build snapshots | Generated artifacts |
| `11c5eb2ae3d4` | Refresh stale core fixtures | Coverage |
| `2d1d45480812` | Persist thread user lifecycle state | Session identity |
| `a35ce79899a4` | Use live config for title suggestions | Titles |
| `6714c9456384` | Add transcript browser prototypes | Transcript views |
| `d9aaee7ac411` | Pass loader overrides from standalone exec binary | Local runtime |

## Post-merge follow-up inventory

These local commits came after the May 20 merge anchor. They are not needed to
reconstruct the pre-merge fork delta, but their lasting behavior belongs in the
current fork contract.

| Commit | Local change | Spec area |
| --- | --- | --- |
| `406381099e5c` | Remove stale reload-exit handling after the merge | Merge repair |
| `ae69128ee2eb` | Archive numbered local Codex builds | Local runtime |
| `62e3ef8067cc` | Drop stale merged state-test import | Merge repair |
| `d493d92da968` | Remap conflicting old local state migrations | State compatibility |
| `e6dda1d1ee04` | Advance the local build marker after the archived build | Local runtime |
| `7c89664f2092` | Restore split footer status, compact agent picker, and direct edit shortcuts after merge | Footer, agents, editing |
| `fafeb52f2fc3` | Advance the local build marker after `v60` verification | Local runtime |
