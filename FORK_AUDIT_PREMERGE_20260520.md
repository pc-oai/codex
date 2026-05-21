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
touches 312 files with 17,863 insertions and 1,497 deletions. Of the 265 old
fork paths checked in the accounting pass, 253 also appear in that current fork
delta.

The only files added by the old fork range that are absent from current `HEAD`
are the old state migration filenames:

- `codex-rs/state/migrations/0030_threads_user_message_count.sql`
- `codex-rs/state/migrations/0033_threads_user_message_count_known.sql`
- `codex-rs/state/migrations/0034_threads_user_state.sql`

That absence is expected only because the post-merge state repair remapped the
fork's conflicting migration numbers. Keep checking the state behavior and the
remap path; do not restore those filenames blindly.

These old touched paths did not appear in the first current fork-delta path
set. Keep the distinction between expected refactors and remaining checks:

| Old path | First-pass disposition |
| --- | --- |
| `app-server-protocol/schema/json/v2/Item{Started,Completed}Notification.json` | Old delta only changed trailing newline state. |
| `app-server-protocol/src/protocol/v2.rs` | Upstream split v2 into modules; local thread config/delete/user-state/subagent fields now live under `protocol/v2/{config,thread,thread_data}.rs`. |
| `thread-store/src/remote/{helpers,list_threads,mod}.rs` | Old remote-store edits only filled new thread fields and rejected remote delete; current thread-store no longer has that remote module. Check current thread delete/user-state behavior through app-server and active store paths instead. |
| `tui/src/history_cell.rs` | Upstream split history cells into `history_cell/`; condensed mode, session header, timing, and tests live in the split modules now. |
| old history tooltip and resume-picker table snapshots | Snapshot layout moved with the split history cells and picker reshaping. Review current snapshots when picker/chrome behavior changes. |
| `tui/src/status_indicator_widget.rs` | Real missing Talon path found in this audit pass: the app bridge and live task summary setter had fallen out even though `talon.rs` and helper binaries still existed. |
| `tui/src/tui.rs` | Old thread-switch viewport clearing moved: current `app/session_lifecycle.rs::clear_terminal_for_thread_switch` clears scrollback directly during switch reset instead of deferring a scheduled clear through `Tui`. |
| `tui/src/app/side.rs` | Real missing line found in this audit pass: side-thread discard must refresh full agent activity label, not only the active label. |

`resume_picker.rs` was present on both sides of the merge and still needed a
behavior audit. This pass found and restored the old retrieval cues and fast
paths inside the reshaped picker: custom-title filtering and emphasis, message
counts, compact cwd labels, the short ID suffix, local open-session detection
through Talon sockets, and the warmed all-directories page used by `Ctrl-A`.

The next keymap pass found the same shape of regression for agent rotation:
`rotate_agent_shortcut_matches` still existed after the merge, but current app
input only routed the explicit next-agent bracket shortcut. `Ctrl-S` belongs to
the fixed forward-agent path from the old fork, so new configurable actions
must not take it as a default binding.

### Current `HEAD` static anchors

| Fork behavior | Current `HEAD` evidence found |
| --- | --- |
| Talon session sockets | `TalonSocketRequest`, `start_socket_acceptor`, `talon_ambient_state`, `handle_talon_request` |
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

### Compound commit decomposition

Commits `615a82f60acc` and `820635042674` are too broad to audit by subject
line. The first feature-level pass split them this way:

| Old commit | Behavior groups checked | Current evidence |
| --- | --- | --- |
| `615a82f60acc` session workflows | TUI runtime toggles (`--private`, `--config-file`, no MCP, no project docs), thread delete and persisted message-count retrieval metadata, saved-cwd resume/reload/fork flow, `/delete` `/reload` `/id` copy/title helpers, compact status/history chrome, and title suggestion workers | CLI runtime toggle tests, app-server thread-delete tests, thread metadata/store tests, `session_resume` cwd tests, slash/title suggestion tests, status/footer snapshots, and the current session/picker rows below |
| `820635042674` context and composer controls | Unsafe context-window override, compact context-used status variants, rename/retitle and queue edit/discard/steer keymap actions, queued hint binding refresh, edit-last composer accent, and the reverse-history remap away from reload/agent chords | model override tests, status surface tests, keymap action/default/conflict tests, queued-message tests and snapshots, edit-last accent snapshot/tests, and the `Ctrl-R`/`Ctrl-S` reservation now stated in `FORK_SPEC.md` |

This decomposition is still a regression checklist, not a proof by ancestry.
When any one of these areas changes later, rerun its focused tests and its live
TUI check if the behavior is terminal-visible.

### Requirement evidence

This table accounts for the behavior contract against the clean current `HEAD`.
Code anchors prove that the merged tree still has the surface; tests or terminal
checks are stronger evidence for the paths most likely to lose wiring during a
merge.

| Contract area | Current code evidence | Current verification evidence |
| --- | --- | --- |
| Composer clears, edit-last, queue controls | `clear_composer_draft`, `edit_last_message_from_command`, queued edit/discard/steer keymap actions | `ctrl_x_clears_composer_without_emitting_ops`, the edit-last preview/commit/cancel app tests, queued-message binding tests under `chatwidget/tests/composer_submission.rs`, private local build `v68` `Ctrl-E` smoke covering preview, `Esc` displaced-draft restoration, and rollback resubmit, `previous_message_edit_mode_uses_distinct_composer_accent`, remote `footer_mode_edit_last_message_snapshot`, and local build `v71` pencil edit-preview smoke |
| Session selector lookup and picker retrieval | `SessionSelectorLookup`, `thread_id_contains_normalized_fragment`, `resume_picker.rs` title/message/cwd/open-state paths | `short_session_id_fragments_match_across_uuid_hyphens`, `cargo test -p codex-tui resume_picker`, a source-build ambiguous-fragment terminal check that opened the narrowed picker, a unique-fragment check that resumed directly, and local build `v68` `resume --all` showing dense relative-time columns with live `open` indicators |
| Thread delete and user work state | app-server `thread/delete`, `ThreadUserState`, local thread-store message-count overlay | `thread_delete_removes_materialized_rollout_and_sqlite_metadata`, `thread_metadata_update_sets_user_state_and_thread_read_preserves_it`, and local thread-store user-message-count coverage |
| Titles and terminal identity | title suggestion app jobs, `/retitle`, `/emoji`, terminal-title session suffix formatting | title suggestion event-path tests, slash-command title tests, and `terminal_title_preview_uses_session_id_suffix_for_live_values` |
| Saved cwd, fork boundary, resume replay | saved-session cwd resolution, fork truncation, saved exec replay, reload handoff | thread fork tests for last-completed-turn behavior, `replayed_completed_exec_turn_rehydrates_tool_output`, reload handoff tests, app draft replay tests, and a source-build `Ctrl-R` smoke that restored a non-empty draft after re-exec |
| Rollback edit flow | rollback turn start payloads and `ThreadRolledBackNotification` routing | core rollback reconstruction tests plus app edit-last rollback tests |
| Config and local CLI controls | `model_context_window_allow_unsafe_override`, `paste_text_inline_char_limit`, CLI `--private`, no-MCP/no-project-docs/config-file loader overrides | config/model tests, paste constructor coverage, CLI private flag coverage, and `constructor_applies_paste_text_inline_char_limit` |
| Transcript and startup chrome | `ToggleCondensedTranscriptView`, suppressed startup header history, right-side footer/MCP status | condensed reflow and restored scrollback tests, clear-header snapshots, status/footer snapshots, and the `Alt-C` terminal smoke already done for archived local build `v64` |
| Agent navigation and tree restore | agent menu/prewarm, `Ctrl-S`, `Alt-[`, and `Alt-]` routing, resume subagent tree, reload tree handoff | agent prewarm tests, active-agent footer tests, `agent_shortcut_matches_explicit_rotation_bindings`, keymap fixed-shortcut conflict tests, `thread_resume_params_forward_tree_restore_flag`, `reload_exit_thread_id_prefers_primary_thread_for_tree_handoff`, subagent tree resume coverage, source TUI `v78` `Alt-\` plus `Ctrl-S` smoke that switched an idle spawned child back to main, and local build `v74` selected-child reload smoke with the restored child draft confirmed through its Talon socket |
| Talon per-session control | `start_socket_acceptor`, delayed startup retry in `App::run`, `talon_ambient_state`, status-row summary bridge | Talon socket unit tests and a live source-build `command.sock` smoke on `v65`: `get_state`, `set_buffer`, and socket-triggered exit all returned responses |
| Runtime/build visibility | `scripts/local-build-codex`, local build footer label, Ghostty OSC progress path, runtime metrics surfaces | local build snapshots, terminal progress unit tests, runtime metrics tests, full TUI snapshots, live source-build progress start/clear OSC bytes, and local build `v74` Ghostty captures showing the native top-surface progress bar during a real `sleep 60` turn and no bar after completion |
| State compatibility | remapped old local applied migrations before current state migrator | `remaps_conflicting_local_migrations_before_current_state_migrations` and state runtime migration coverage |
| Local experiments/runtime propagation | transcript outline/tree binaries and standalone exec loader override propagation | current binaries/code anchors plus CLI/exec loader override callsites |

### Current checkout

The path and requirement accounting above was rerun after the recovery commits
with a clean worktree. The current delta is larger than the old fork delta
because it includes post-merge repairs and later local additions such as idle
thread-spawn and runtime-metrics persistence; those additions do not replace the
pre-merge checks in this report.

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

1. Reuse the compound-commit decomposition above when session
   lookup/delete/state, title flows, startup/footer chrome, keymap, context, or
   composer controls change again.
2. Keep rerunning live TUI checks when later repairs touch the behavior:
   current checks cover the Talon socket round trip, short selector
   unique/ambiguous lookup, reload with a non-empty root draft,
   selected-subagent reload handoff, edit-last rollback/cancel/submit plus the
   pencil edit preview, Ghostty visual progress, and terminal progress
   start/clear bytes.
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
