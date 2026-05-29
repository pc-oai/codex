# Local Codex fork journal

Legend:

- `✓` done
- `⌕` investigated
- `◇` discussed
- `·` note

## 2026-05-04

- ✓ Rebased the fork onto current upstream and repaired the local branch after
  the rebase.
- ✓ Moved startup work off the critical path so the TUI can accept text sooner.
- ✓ Kept bundled-model startup with background model refresh.
- ✓ Made local runtime metrics visible without requiring a remote metrics
  exporter.
- ✓ Added interactive private/no-history mode as `--private` / `-P`.
- ✓ Split fork-only config out so production Codex does not choke on local-only
  config keys.
- ✓ Started the thread-ID, terminal-title, Talon, and reload-draft automation
  thread.
- ◇ Deeper startup work: single-pass config loading and deferring more setup
  work.

## 2026-05-05

- ✓ Made custom-titled sessions stand out in the resume picker.
- ✓ Added picker hotkeys for titled sessions and current-directory/all-directory
  scope.
- ✓ Added session message counts, historical count backfill, compact cwd labels,
  and better table disambiguation.
- ✓ Built title and emoji flows that use whole-conversation context without
  polluting the visible transcript.
- ✓ Added or refined local session helpers such as `/delete`, `/reload`, `/id`,
  and copy helpers.
- ✓ Made resume, reload, and fork prefer the saved session cwd when it still
  exists.
- ✓ Started the direct edit-last-message shortcut path that became reversible
  `Ctrl-E` editing.
- ✓ Kept prior-turn timing visible in muted form while the next turn runs.
- ◇ Session colors and eventual Ghostty tab-color sync.
- ◇ Auto-title after the first turn.
- ◇ Richer in-place history editing.

## 2026-05-06

- ✓ Wrote repo-root fork notes so local behavior and rationale survive rebases.
- ✓ Committed the first large local session-workflow bundle.
- ✓ Finished fork behavior around saved cwd and stable completed-turn
  boundaries.
- ✓ Made edit-last-message preview safer and clearer: cancel versus commit,
  visible edit state, repeated `Ctrl-E` walking backward.
- ✓ Fixed `/retitle` and `/emoji` when the hidden title worker completes with
  an empty final completion envelope.
- ✓ Continued local slash, alias, Talon, and SpeechLab command work around
  naming, reload, effort, copy, and ID surfaces.

## 2026-05-07 through 2026-05-12

- · No separate exact-cwd `codex-rs` raw sessions in this audit window.
- · Some earlier long sessions continued to receive later turns, but there is
  no distinct exact-cwd day entry here.

## 2026-05-13

- ✓ Made `Ctrl-E` edit-last-message work even when the composer already
  contains a draft.
- ✓ Restored the displaced draft when edit mode is canceled.
- ✓ Extended draft preservation beyond reload toward ordinary quit/resume
  flows.
- ⌕ Investigated edit-after-resume and edit-after-fork behavior.
- ⌕ Investigated edit-submit-edit-again behavior.

## 2026-05-14

- ✓ Added an open/closed signal to resume picker rows.
- ✓ Used live local session sockets for local open-state detection.
- ✓ Made terminal titles collapse to the useful title/session-ID form when a
  session has a real title.
- ⌕ Traced a stale local binary when the new picker behavior did not appear
  live.

## 2026-05-15

- ⌕ Diagnosed a Codex session that looked hung but had already ended its turn
  via interruption in the session JSONL.
- ⌕ Separated model-turn state from a TUI/input-loop hang shape.
- ⌕ Located local panic and crash evidence paths for iterating on the fork.
- · No fork code change landed that day.

## 2026-05-16

- ✓ Added faster multi-agent navigation: `Ctrl-S`, `Alt-[`, and `Alt-]`.
- ✓ Preserved drafts with the agent thread where they were typed.
- ✓ Added feedback when there is no other agent to switch to.
- ✓ Built the floating `/agent` menu that leaves the composer visible.
- ✓ Polished the floating menu layout and fixed its short-buffer bounds crash.
- ✓ Made composer `Ctrl-A` open the agent menu only when line-start movement
  has no more work to do.
- ✓ Shaped reload around the whole agent tree instead of only the currently
  viewed child thread.
- ◇ Resume-picker presentation for agent trees and descendants.

## 2026-05-17

- ✓ Added Ghostty terminal-native progress while Codex turns run.
- ✓ Made the progress bar configurable.
- ✓ Added condensed transcript behavior in the main terminal view.
- ✓ Fixed restoring hidden tool output when returning from condensed mode.
- ✓ Fixed resume replay so persisted `exec_command` output reappears in
  history.
- ✓ Added bounded prewarm work for nearby agent targets.
- ⌕ Investigated first-switch agent latency.
- ◇ Tree-style transcript browsing after the flat transcript view felt too
  hard to navigate.

## 2026-05-18

- ✓ Made large dictated pastes stay inline longer through
  `paste_text_inline_char_limit`.
- ✓ Restored audible completion behavior locally with explicit BEL notification
  config.
- ✓ Designed and implemented saved thread user state: `active`, `parked`, and
  `done`.
- ✓ Split a large dirty TUI stack into coherent commits covering paste
  threshold, Ghostty progress, rollback edit support, subagent tree resume and
  reload, condensed transcript, shortcut/keymap refresh, Talon socket work,
  short session selectors, agent menu/prewarm, and title/status cleanup.

## 2026-05-19

- ✓ Fixed the retitle crash path by removing the optional config reload that
  overflowed before the hidden title worker starts.
- ⌕ Checked mid-turn retitle behavior and confirmed ephemeral-fork semantics
  were not the real crash cause.
- ⌕ Estimated the upstream Markdown table backport.
- ◇ Left Markdown table integration for later.
- ✓ Refined transcript outline/tree prototype direction.
- ◇ Brainstormed local CLI names without choosing one.
- ✓ Committed user-state, live-config title suggestions, transcript prototype
  binaries, and standalone exec loader override propagation before the merge.

## 2026-05-20

- ✓ Merged upstream into the local fork.
- ✓ Inspected the local state repair prompt and remapped conflicting local state
  migration numbers.
- ✓ Added numbered local build archives so old local binaries remain runnable.
- ✓ Created and expanded `FORK_SPEC.md` as the fork contract.
- ✓ Preserved the pre-merge local tip and audited old-versus-merged fork
  behavior.
- ✓ Restored post-merge regressions in footer, edit, startup, shortcut, picker,
  and Talon behavior.
- ✓ Added user-spawned child subagents through app-server and TUI surfaces.
- ✓ Added idle spawned-subagent flow and attach/select fixes.
- ✓ Restored paste-threshold constructor wiring after the merge.
- ✓ Pinned condensed transcript `Alt-C` behavior and smoke-tested it in a local
  archived build.
- ✓ Quieted runtime timing noise in the transcript while persisting metrics in
  session JSONL.
- ⌕ Investigated remote build offload and devbox/AWS Mac build performance.

## 2026-05-21

- ✓ Continued the May 20 merge audit and recovery work.
- ✓ Restored reload handoff consumption on resume after the audit found it
  missing.
- ⌕ Verified picker selector behavior, Talon socket behavior, and reload draft
  smoke paths during fork recovery.
- ✓ Restored readable resume-picker relative dates in the current working
  stack.
