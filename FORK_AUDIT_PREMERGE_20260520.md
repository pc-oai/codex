# Pre-merge fork audit: 2026-05-20

This audit is the accounting baseline for the local Codex fork before the May
20 upstream merge. Use it with `FORK_SPEC.md`: the spec is the product contract,
while this file records the historical delta and the first comparison against
the merged tree.

## Range

| Item | Revision |
| --- | --- |
| Pre-merge fork tip | `pc/local-main-before-upstream-merge-20260520` / `d9aaee7ac411` |
| Merge commit | `fa30bac836a3` |
| Upstream side of that merge | `fa30bac836a3^2` / `c3faea0b09ca` |
| Old fork merge base | `7e71d0261027` |
| Old fork range | `7e71d0261027..d9aaee7ac411` |
| Clean old-fork worktree used for this pass | `/Users/pc/code/worktrees/codex-premerge-audit-20260520` |

The old fork range contains 33 commits. Its tree delta touches 265 files with
14,816 insertions and 1,523 deletions. TUI code and snapshots dominate the
range, followed by app-server thread APIs/schema, state/thread-store metadata,
config, and local automation.

Do not use ancestry alone to declare this audit complete. The old fork tip is
an ancestor of current `main` through the merge, but merge resolution and later
refactors can still drop wiring or leave an old binary running. The paste inline
limit and the `Alt-C` transcript toggle already demonstrated both failure modes.

## Pre-merge commit accounting

| # | Commit | Local change | Audit area |
| --- | --- | --- | --- |
| 1 | `7ea001a04639` | `Ctrl-X` composer clear | Composer |
| 2 | `7101a98f8053` | Talon file RPC support | Local automation |
| 3 | `74038d1d3688` | WIP save before upstream rebase | Compound baseline |
| 4 | `b37de3f14760` | Rebase fallout and startup repairs | Compound baseline |
| 5 | `facd3e146d69` | Local fork notes | Maintenance |
| 6 | `26f8024eaea0` | Expanded local fork notes | Maintenance |
| 7 | `615a82f60acc` | Session workflow expansion | Session/app-server/TUI compound change |
| 8 | `280f8091903f` | Streamed title suggestions | Titles |
| 9 | `a8766b83b944` | Preserve composer drafts across edit and resume | Editing/reload |
| 10 | `820635042674` | Context override and composer/keymap controls | Config/composer |
| 11 | `7ef47d4bc448` | Resumed exec output history replay | Session replay |
| 12 | `21d96564576e` | Versioned local build helper | Local runtime |
| 13 | `752642e1dd56` | Configurable paste inline limit | Composer |
| 14 | `73d4cc00e9f6` | Ghostty progress during turns | Runtime signals |
| 15 | `a80b5d32bd54` | Rollback turn starts for edit flows | Editing/app-server |
| 16 | `f1dcbb148741` | Open subagent trees on resume | Agents |
| 17 | `8f767d057982` | Reload trees across process restart | Reload/agents |
| 18 | `752ad85ac4db` | Condensed transcript view | Transcript |
| 19 | `7db88571b926` | Composer and session shortcuts | Keyboard workflow |
| 20 | `c629b89bb219` | Per-session Talon sockets | Local automation |
| 21 | `0c2d8d29f01a` | Short session selectors | Session discovery |
| 22 | `5065e5894b1b` | Agent switcher menu and prewarm | Agents |
| 23 | `553fb35368c6` | Thread-aware terminal titles | Titles/chrome |
| 24 | `107bdc559b10` | Queue hint refresh after keymap edits | Coverage/keymap |
| 25 | `1f08362c8cb7` | Durable resumed history replay coverage | Coverage |
| 26 | `0e80674bb199` | Side rename and copy remap edge coverage | Coverage |
| 27 | `94210679c239` | Generated config schema refresh | Generated artifact |
| 28 | `ad5a44589885` | Source-build snapshot refresh | Generated artifact |
| 29 | `11c5eb2ae3d4` | Stale core fixture refresh | Coverage |
| 30 | `2d1d45480812` | Thread user lifecycle state | Session state |
| 31 | `a35ce79899a4` | Live config for title suggestions | Titles |
| 32 | `6714c9456384` | Transcript browser prototypes | Transcript experiments |
| 33 | `d9aaee7ac411` | Loader overrides from standalone exec binary | Local runtime |

`FORK_SPEC.md` has the same pre-merge commit inventory in a product-oriented
form. Commits 3, 4, 7, and 10 are the large audit hazards: they combine
multiple behaviors and should be checked by behavior, not by commit subject.

## Current comparison

The current fork delta from the upstream side of the merge to current `HEAD`
touches 283 files with 15,634 insertions and 1,226 deletions. Of the 263 old
fork paths checked in the accounting pass, 248 also appear in that current fork
delta.

The only files added by the old fork range that are absent from current `HEAD`
are the old state migration filenames:

- `codex-rs/state/migrations/0030_threads_user_message_count.sql`
- `codex-rs/state/migrations/0033_threads_user_message_count_known.sql`
- `codex-rs/state/migrations/0034_threads_user_state.sql`

That absence is expected only because the post-merge state repair remapped the
fork's conflicting migration numbers. Keep checking the state behavior and the
remap path; do not restore those filenames blindly.

These old touched paths are not in the current fork delta path set and need
either an upstream/refactor explanation or a behavior check:

- `app-server-protocol` item start/completion schema and old `v2.rs`
- `thread-store/src/remote/{helpers,list_threads,mod}.rs`
- `tui/src/app/side.rs`
- `tui/src/history_cell.rs`
- two older TUI snapshots for history tooltip and resume-picker table
- `tui/src/status_indicator_widget.rs`
- `tui/src/tui.rs`

Some are expected split-file churn after upstream changes. Keep the list as a
review queue until each behavior is accounted for.

### Current `HEAD` static anchors

| Fork behavior | Current `HEAD` evidence found |
| --- | --- |
| Talon session sockets | `TalonSocketRequest`, `start_socket_acceptor` |
| Short session selectors | `thread_id_contains_normalized_fragment` |
| Condensed transcript | `ToggleCondensedTranscriptView` |
| Ghostty turn progress | `sync_managed_terminal_progress` |
| Unsafe context override | `model_context_window_allow_unsafe_override` |
| Configurable paste inline limit | `paste_text_inline_char_limit` plus constructor wiring |
| Rollback edit flow | `ThreadRolledBackNotification` |
| User lifecycle state | `ThreadUserState` |
| Agent picker/menu | `AgentMenu` |
| Reload handoff | `reload_handoff` |
| Transcript prototypes | `transcript_outline` |
| Footer/MCP split | `set_status_line_right`, `mcp_startup_progress_label` |
| Fast model bootstrap | `local_bootstrap_models` |
| Local title helpers | `ThreadNameSuggestionKind` |
| Standalone exec loader override | `run_main(..., loader_overrides)` in `exec` |

Static anchors are necessary, not sufficient. They do not prove constructor
wiring, key routing, snapshot shape, or the binary Phil is running.

### Current dirty worktree

The current checkout is not a clean comparison target. At the time this report
was written it had 41 tracked dirty files, and 26 of those overlap paths from
the old fork delta. The dirty work includes `FORK_SPEC.md`, session selector and
resume picker work, reload/model handoff work, user/runtime telemetry paths,
subagent paths, timing separator work, and snapshots. Compare against `HEAD`
first; re-run this section after that stack is committed or intentionally
snapshotted.

## Post-merge repairs already observed

Post-merge repair commits explicitly reintroduced or guarded startup chrome,
footer/edit shortcuts, reload and agent rotation, local title/session helpers,
condensed transcript mode, agent prewarm updates, progress/status behavior,
edit and queue workflow, slash aliases, paste threshold wiring, and the
`Alt-C` default binding. `git range-diff` only pairs the condensed transcript
commit cleanly against a post-merge repair; the rest must be checked through
behavior anchors and tests because the merge and upstream refactors changed the
patch shape.

## Next behavior audit

Use this order for the missing-functionality hunt:

1. Check large compound commits `615a82f60acc` and `820635042674` by feature:
   session lookup/delete/state, title flows, startup/footer chrome, keymap and
   composer controls.
2. Run live TUI checks for the behaviors that a static anchor cannot prove:
   Talon socket round trip, short selector unique/ambiguous lookup, reload with
   a non-empty draft and subagent tree, edit-last rollback/cancel/submit, and
   Ghostty progress.
3. Use focused tests and snapshots around app-server thread payloads, thread
   state migrations/store, resume picker, footer/status lines, queue/edit UI,
   paste placeholders, and condensed transcript mode.

Useful range commands:

```sh
git log --reverse --oneline \
  7e71d02610272829e96a50253948fdbfc679db0f..d9aaee7ac41186e264dea6b72c9cfe7530fb7eb2
git diff --stat \
  7e71d02610272829e96a50253948fdbfc679db0f..d9aaee7ac41186e264dea6b72c9cfe7530fb7eb2
git range-diff \
  7e71d02610272829e96a50253948fdbfc679db0f..d9aaee7ac41186e264dea6b72c9cfe7530fb7eb2 \
  fa30bac836a3b347922c37200479f07444a82a12..HEAD
```
