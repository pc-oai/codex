# Local fork specification

This file is the behavioral contract for Phil's local Codex fork. Use it when
an upstream merge or rebase has a large conflict surface and it is not enough
to see that the tree compiles.

`FORK_NOTES.md` is the short running note of why these divergences exist. This
file is the fuller motivation, pre-merge inventory, and post-merge validation
guide.

## Keyboard shortcuts quick list

Update this list first when the fork's local key bindings change. Keep it short
and easy to scan.

- `Ctrl-R`: reload the current session in a fresh process.
- `Ctrl-X`: clear the composer or discard the relevant queued draft.
- `Ctrl-E`: open edit-last-message when the composer cursor is already at line
  end.
- `Alt-Up` / `Alt-E`: open queued-message edit.
- `Alt-Down` / `Alt-Enter`: steer the newest queued follow-up immediately.
- `Alt-R`: open manual rename for the current session.
- `Ctrl-Shift-R`: request a fresh title suggestion for the current session.
- `Ctrl-A`: open the compact agent menu when line-start movement has no more
  work to do.
- `Ctrl-S`: rotate forward through agent threads.
- Session pickers use `Ctrl-A` to switch between current-directory sessions
  and all directories when a cwd filter exists.
- Session pickers use `Ctrl-T` to filter to custom-titled sessions, and
  `Ctrl-Q` / `Ctrl-X` exit the picker like `Ctrl-C`.
- Session pickers keep `Ctrl-P` for opening the selected session transcript
  after `Ctrl-T` is reserved for title filtering.
- `Alt-C`: toggle condensed transcript mode.
- `Alt-[`: switch to the previous agent thread.
- `Alt-]`: switch to the next agent thread.
- `Alt-\`: spawn an idle child agent from an empty composer and switch to it.


## Feature areas

Dev:
- fast reload (Ctrl-R, Say, preserves model)
- Better restoration of state (tool outputs, turn, runtimes, and so forth)

Composer:
- clear composer / discard queued draft (`Ctrl-X`)
- edit-last (`Ctrl-E`; stash current draft; repeat to walk backwards; make the
  input box visibly yellow with a pencil icon while editing)
- queued follow-up edit (revise the next waiting message)
- queued follow-up steer / discard (send the latest one as an immediate steer, or drop it)
- configurable inline paste threshold (inline text vs placeholder)
- local slash helpers (for example `/id` and `/reload`)

Session restore:
- resume / fork by UUID, ID fragment, or name
- picker retrieval cues (title, cwd, ID suffix, open state)
- saved cwd restore (resume, reload, and fork)
- replayed saved command output (after resume)
- active / parked / done state (separate from archive/delete)

Titles:
- manual rename (`Alt-R`)
- retitle helper (`/retitle`)
- retitle shortcut (`Ctrl-Shift-R`)
- emoji helper (`/emoji`)
- thread-aware terminal title (session ID item uses the ID suffix)

Agents:
- compact agent menu
- prev / next switching (`Alt-[` / `Alt-]`)
- short footer neighbor strip while switching
- direct subagent spawn (`/subagent`, idle `Alt-\`)
- active-agent footer state (count and working marker)
- preserve main draft while switching

Transcript/chrome:
- condensed transcript toggle
- no boxed or transient startup header table
- split status / footer
- compact turn timing
- transcript browser experiments
- optional Ghostty progress

Reload/handoff:
- reload into fresh process
- preserve working cwd
- preserve drafts / subagent tree
- fork from stable completed turn

Automation/builds:
- per-session Talon control
- session command socket
- invocation-local runtime overrides
- visible local build labels
- archived numbered binaries
- fast model bootstrap

## Feature summary

- Sessions: richer resume/fork lookup, retrieval cues in pickers, saved cwd and
  user work state, stable reload/resume handoffs, and replay of durable work
  output.
- Titles: local naming, retitle, and emoji flows keep threads and terminal tabs
  recognizable without polluting the transcript.
- Editing: reversible edit-last-message, composer-draft preservation across
  edit-last, agent switching, reload, and resume, queue controls, configurable
  key bindings, paste visibility, and local slash helpers keep repeated prompt
  work fast without making it lossy.
- Agents: compact picker/switching/prewarm behavior, visible active-agent
  state, direct user-spawned subagents, and side conversations support many
  active threads in one TUI.
- Transcript and chrome: condensed scrollback, local transcript experiments,
  no startup session-header table, split
  status/footer surfaces, compact timing output, and optional Ghostty progress
  keep runtime state visible without transcript noise.
- Automation and builds: per-session Talon control, invocation-local runtime
  overrides, visible local build numbers, and archived numbered binaries keep
  local dogfooding reproducible.

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

Older local backup refs exist outside that May merge line. In particular,
`pc/backup-local-main-2026-02-11` diverges before the May tag: its older Talon
surfaces were reworked into this contract later, while its backup-only 2025
textarea undo stack and manual Option-key escape decoder are historical unless
they are deliberately revived. Do not treat every backup-only experiment as a
May merge regression.

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
  render starred and bold, leading emoji in titles survive display, and picker
  `Ctrl-T` filters to custom-titled sessions.
- Picker rows carry fast retrieval cues: a user-message count, compact cwd
  labels with disambiguation when basenames collide, a short session-id suffix,
  an open/closed signal when that state is known, and readable relative times
  with words such as `35 minutes ago` instead of compact unit suffixes. Dense
  picker rows keep their retrieval cues in separate columns and call out open
  sessions with a live indicator instead of repeating the normal closed state.
- All-directory lookup stays one keypress away with picker `Ctrl-A`, which
  toggles between the current cwd and all directories without first focusing the
  toolbar. That query stays warmed so common picker toggles do not feel like a
  cold fetch.
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
- Composer drafts are preserved across edit-last, agent switching, reload, and
  resume. Edit-last-message can temporarily replace a non-empty composer draft,
  cancel restores the displaced draft, agent navigation keeps the main-thread
  draft intact, and reload handoff preserves enough state that a rebuild test
  does not throw away active draft work.
- Edit-last-message remains reversible before commit. The idle shortcut opens
  a preview, `Esc` cancels it, and submission performs rollback and resubmit.
  Once submission is accepted, the yellow editing surface closes immediately;
  it reopens with the edited draft only if rollback fails. `Ctrl-E` can claim
  the shortcut when line-end movement would be a no-op, but normal editor
  movement still wins while the cursor is inside a draft.
- App-server turn start supports rollback-driven edit flows, and the TUI routes
  rollback notifications through its thread state instead of losing the edit
  context during resume or side switching.
- Open subagent trees survive app-server resume and TUI reload restart.

### Composer and keyboard workflow

- `Ctrl-X` clears the composer input or discards the relevant queued draft
  without borrowing `Ctrl-C`.
- Main-surface key bindings include `Ctrl-R` to reload the current session and
  resume it in a fresh process, plus `Alt-C` to toggle condensed transcript
  mode so tool-call rows and outputs can be hidden from scrollback. Keymap
  config is forward-compatible at load time so an older local binary ignores
  unknown future action names while generated schema stays strict for
  validation.
- `Ctrl-R` and `Ctrl-S` stay reserved for reload and forward agent rotation.
  Reverse-history search must not reclaim those two chords by default.
- Rename, retitle, queued-message edit, queued-message discard, queued-message
  steer, and reverse-history bindings remain configurable local workflow
  actions. The local defaults keep manual rename on `Alt-R`, retitle
  suggestion on `Ctrl-Shift-R`, and immediate queued-message steer on both
  `Alt-Down` and `Alt-Enter`.
- Live keymap edits update queued-input footer hints from the newly resolved
  bindings. A picker that changes the edit binding must not leave the queued
  preview advertising an older default.
- Local slash helpers keep repeated dogfooding work close to the session:
  `/id`, `/copy-last-request`, `/delete`, `/effort`, `/reload`, and the
  lifecycle markers `/active`, `/park`, and `/done`.
- Slash autocomplete keeps short command aliases visible alongside their long
  spellings, including aliases such as `/m`, `/e`, `/r`, `/c`, `/i`, `/rev`,
  `/t`, `/rt`, and `/em`. A typed exact alias wins over longer prefix matches,
  so `/i` selects `/id` rather than `/ide`.
- The local default queued-message edit bindings keep `Alt-Up`, `Alt-E`, and
  `Ctrl-E`; the direct letter shortcuts must not be lost when inherited
  arrow-key paths move.
- Pasted text stays intact for submission while the inline display threshold is
  configurable with `paste_text_inline_char_limit`.
- Local model experiments can opt in to a context window above the catalog max
  with `model_context_window_allow_unsafe_override`.

### Transcript views and agents

- The local TUI has a condensed message-only transcript mode for terminal
  scrollback. `Alt-C` switches between the full transcript and that compact
  view without dropping hidden tool-call history.
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
- `Ctrl-S` rotates forward through the same agent cycle for the fast one-key
  path.
- Keyboard agent rotation flashes a short footer neighbor strip before the
  target thread takes over, so a fast switch still leaves visible context about
  where the cycle moved.
- Active-agent footer labels refresh when a new subagent thread starts so the
  visible count and working-state marker do not lag behind the picker.
- Loaded parent threads can spawn a child agent directly through app-server
  `thread/spawn`. `/subagent <task>` starts a child without switching focus.
  Empty-composer `Alt-\` spawns an idle child and switches into it so the next
  draft belongs to that child. Spawned children inherit the parent runtime
  context, stay in its agent tree, and become visible in the agent picker.

### Local automation and runtime signals

- Source builds go through `scripts/local-build-codex` or `just local-build`
  so the visible local build number advances. Numbered local binaries are
  archived under `target/local-builds/vN/codex` so a known local build can be
  started again directly.
- Local CLI/runtime overrides remain available for dogfooding wrappers,
  including config-file selection, no-MCP, no-project-docs, private mode, and
  loader override propagation into the standalone exec binary.
- Fresh local TUI startup can paint the composer before the first
  app-server `thread/start` result and startup skills refresh arrive. Initial
  thread events stay gated until that thread is configured so early input is
  accepted without racing replay or turn state.
- Fresh local TUI startup uses bundled model data for the first in-process
  render instead of waiting on `model/list`. It refreshes models from the app
  server in the background and updates model-dependent UI after that result
  arrives.
- Fresh local TUI startup and clear-screen redraws do not print the boxed
  `OpenAI Codex` session header into terminal history. The prompt and compact
  footer/status surfaces are enough; the large model/directory/permissions
  banner is startup noise in the local loop. A newly started thread must not
  briefly render a placeholder version of that table while it is configuring.
  The accepted `tui.compact_session_header` setting does not re-enable one.
- The Talon integration is per session. The live TUI control seam is the
  session-owned command socket; the state file remains ambient output for the
  same session. Together they expose editor/session state and narrow control
  actions. The socket control surface includes model selection,
  rename/retitle/emoji operations, edit/reload actions, copying recent requests
  or responses, interruption, and local build number visibility.
- The Talon state response carries the live task summary from the rendered
  status row while a turn is running, not only a busy flag.
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
  `tui.terminal_progress_bar` is enabled. The switch defaults on. Progress is
  cleared on completion or widget drop and is handed across thread-widget
  replacement to avoid flicker.
- Runtime metrics stay available locally when runtime metrics are enabled even
  if no remote exporter is configured.
- Runtime-metrics transcript output stays compact. Do not regress to verbose
  per-turn timing lines or separators that expand websocket/local-tool totals
  into long transcript rows when a concise timing summary is enough.
  - With compact timing enabled, the transcript timing line is one bullet:
    `• Timing: <metric><duration>[  <metric><duration> ...]`. The default
    metrics are TTFT and TBT with the default symbols, for example
    `• Timing: 492ms  ≋11ms`. Do not show both iapi and service values or
    append `(iapi)` / `(service)` labels in this compact line.
  - The status-line `timing` item uses the same compact metric body without the
    `Timing:` prefix, for example `492ms  ≋11ms`; the previous turn can remain
    as a muted footer reference until the current turn reports timing.
  - With compact timing enabled, a final separator keeps the worked duration
    and at most a compact tool summary, for example
    `─ Worked for 22m 59s • 153 tools 752.7s ─`. Do not add websocket send
    counts, websocket receive counts, or expanded TTFT/TBT detail to that
    separator.

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
| Fast model bootstrap | `local_bootstrap_models`, `startup_models_list_with_request_handle`, `ModelsLoaded` |
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

These local changes came after the May 20 merge anchor. They are not needed to
reconstruct the pre-merge fork delta, but their lasting behavior belongs in the
current fork contract. Keep this table grouped by behavior instead of turning
it into a changelog; `FORK_AUDIT_PREMERGE_20260520.md` carries the detailed
recovery evidence.

| Commit(s) | Local change | Spec area |
| --- | --- | --- |
| `406381099e5c` | Remove stale reload-exit handling after the merge | Merge repair |
| `ae69128ee2eb` | Archive numbered local Codex builds | Local runtime |
| `62e3ef8067cc` | Drop stale merged state-test import | Merge repair |
| `d493d92da968` | Remap conflicting old local state migrations | State compatibility |
| `7c89664f2092` | Restore split footer status, compact agent picker, and direct edit shortcuts after merge | Footer, agents, editing |
| `b227bf5118cf` | Suppress the durable boxed startup history header | Transcript/chrome |
| `fd38c1e6b348`, `8aaf14488562`, `73c685cf724f` | Restore reload and agent rotation shortcuts, local title/session helpers, and condensed transcript mode | Keyboard workflow, titles, transcript |
| `14a6b2800039`, `b2da512dcf40`, `3dd21ecbc80`, `21e726dcd153`, `422f82ea323f`, `d5160d052209` | Restore agent prewarm activity updates and switch feedback, progress/status surfaces, edit/queue workflow, and slash alias selection | Agents, runtime signals, editing |
| `3a0cd90f4079`, `4ef824df2aa0`, `89a39f8b55f8` | Add direct and idle user-spawned subagents | Agents |
| `b11052807110` | Persist runtime metrics without bringing verbose timing rows back into the transcript | Runtime signals |
| `3b96a817ff6b`, `82be3373b6e1`, `99e7f29d0d22`, `b655d9f39145`, `ad05bd3795a1` | Restore recovery wiring around reload drafts, picker retrieval cues, edit preview accent, and selected subagent-tree reload | Session restore, editing |
| `e490ff82a8d9`, `6bce2cb81a3e` | Add isolated local target-dir support and the remote build offload guide | Local runtime |
| `9bed483d8452`, `0501917bd33`, `5d061edc3bce` | Keep fixed agent/rename shortcuts from being reclaimed by newer defaults | Keyboard workflow |
| `eeef0758b169`, `ebea69832eab`, `f9d543d1bf08` | Restore async fresh startup, bundled-model bootstrap, and compact configuring placeholder | Startup/chrome |
| `90249ad37e71`, `319daa6ed213`, `5a7c58dda99b` | Remove the observed transient startup-table flash and close edit mode when an edited submit is accepted | Startup/chrome, editing |
| `f90453686afb` | Cover standalone exec loader override propagation | Local runtime |

## Short session-ID suffix contract

The fork treats the tail of a session ID as a practical retrieval handle. Do
not regress this to prefix-only IDs: the suffix is the compact ID Phil expects
to spot and type.

- `resume` and `fork` selectors accept an exact UUID, a thread name, or a
  hexadecimal ID fragment.
- Fragment matching is not prefix-only. It ignores UUID hyphens, normalizes
  case, and matches anywhere inside the session ID, so a pasted suffix works.
- Exact UUID or name lookup wins first. A unique ID fragment opens that thread;
  an ambiguous fragment opens a narrowed picker instead of choosing silently.
- Resume-picker filtering understands ID fragments. Picker rows and expanded
  details use `…` plus the final eight session-ID characters instead of the
  full UUID.
- Terminal-title session-ID output uses that same compact suffix shape:
  `…xxxxxxxx`, not the leading UUID characters.
- The suffix form is intentional. It is the more useful quick discriminator in
  local session lists and terminal chrome than repeated-looking prefixes.
