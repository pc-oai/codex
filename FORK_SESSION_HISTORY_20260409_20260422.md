# Codex fork session history: 2026-04-09 through 2026-04-22

This is the raw-session-history window immediately before the April 23 through
May 6 audit. It counts Codex session files that contain a `session_meta` record
whose cwd is exactly:

```text
/Users/pc/code/codex/codex-rs
```

The April window is small. It does not contain the beginning of local Codex
fork work. Current Git refs preserve older fork commits, so this note also
records the evidence boundary for when the local work appears to have started.

## Inventory

| Raw-session day | Matching files | Accounting note |
| --- | ---: | --- |
| 2026-04-09 | 2 | Stream Deck and CPU diagnostics launched from the Codex cwd. |
| 2026-04-10 | 0 | No exact-cwd `codex-rs` raw session. |
| 2026-04-11 | 0 | No raw session directory for this day. |
| 2026-04-12 | 0 | No exact-cwd `codex-rs` raw session. |
| 2026-04-13 | 1 | Talon/Ghostty/Codex title and dictation diagnosis. |
| 2026-04-14 | 0 | No exact-cwd `codex-rs` raw session. |
| 2026-04-15 | 0 | No exact-cwd `codex-rs` raw session. |
| 2026-04-16 | 1 | Fork-main cleanup and Talon file-RPC restoration. |
| 2026-04-17 | 0 | No exact-cwd `codex-rs` raw session. |
| 2026-04-18 | 0 | No exact-cwd `codex-rs` raw session. |
| 2026-04-19 | 0 | No raw session directory for this day. |
| 2026-04-20 | 1 | Upstream `/rename` verification. |
| 2026-04-21 | 0 | No exact-cwd `codex-rs` raw session. |
| 2026-04-22 | 0 | No exact-cwd `codex-rs` raw session. |

## Day by day

### 2026-04-09

Raw files:

- `rollout-2026-04-09T09-46-05-019d7322-fada-72b3-8093-e73caed009fd.jsonl`
- `rollout-2026-04-09T09-48-22-019d7325-1277-7f51-a190-da0e6fadbb4a.jsonl`

These are cwd matches, not Codex fork implementation sessions.

- The `09-46-05` file fixes a `streamdeck-mode custom` path in `~/bin/pc`.
  The command had been claiming custom mode while no Stream Deck device was
  enumerable and its daemon path could crash-loop.
- The `09-48-22` file investigates machine CPU use. The raw thread is host
  triage around terminal and process load, not a Codex code change.

### 2026-04-13

Raw file:

- `rollout-2026-04-13T17-10-10-019d8952-fd92-7770-a0a4-220f016b031a.jsonl`

This session is adjacent to the fork, but most changes are in Talon/config.

- Investigated:
  - Automatic dictation in Ghostty did not reliably recognize Codex.
  - Codex title detection was relying on title shapes that could collapse to a
    plain project name at idle.
  - The active binary was a released DotSlash Codex artifact, not the local
    checkout binary first suspected.
- Implemented outside the Codex repo:
  - Talon terminal/Codex auto-dictation refresh was repaired.
  - The Codex config template was updated to include richer terminal-title
    status items.
- Reasoning:
  - Terminal title alone was not a stable enough recognition signal for Talon
    when Codex idle titles fall back to project-only text.
  - The runtime binary and source checkout need to be distinguished before a
    title problem is attributed to a local fork.

### 2026-04-16

Raw file:

- `rollout-2026-04-16T13-19-20-019d97f2-bb7d-7801-a491-9bf2dcd9fd30.jsonl`

This is the real local-fork maintenance session in the April window.

- Implemented or reconciled:
  - Local change history was summarized and the Ctrl-X PR branch was rebased
    against current upstream.
  - Fork `main` was simplified to the rebased Ctrl-X composer-clear commit.
  - The older Talon command/file-RPC stack was found missing from that clean
    branch and reapplied as one integrated change on current fork `main`.
  - Talon integration was ported into current app/composer APIs rather than
    blindly restoring obsolete composer code.
- Fork behavior restored:
  - `talon-send` and `talon-sim`,
  - request polling and composer buffer/cursor updates,
  - state replies with thread/session id, cwd, task status, and summary,
  - history previous/next and edit-previous command handling.
- Motivation and reasoning:
  - Phil explicitly asked to make the fork `main` simpler after PR cleanup.
  - Once that clean main was found to have dropped Talon command-server work,
    the restore needed integration work against current upstream TUI shape.
- Validation:
  - The raw session reports focused Talon/TUI checks and command-line smokes.
  - A wider TUI suite still had an unrelated resume CLI parser failure.

### 2026-04-20

Raw file:

- `rollout-2026-04-20T14-49-33-019dacde-c408-7fa3-9819-bef8a12c5bee.jsonl`

This is an upstream existence check, not a local feature implementation.

- Fetched remotes and checked whether session retitling already existed.
- Verified upstream current code has thread/session rename behavior exposed as
  `/rename`.
- Stopped at the verification boundary instead of designing a duplicate slash
  command once upstream behavior was found.

## Fork start evidence

This April window shows fork maintenance already in progress. It is not the
start.

### Current May fork contract range

The local-only range preserved by the May pre-merge fork tag starts with:

```text
7ea001a04639  2026-03-12T16:02:29-07:00  tui: add ctrl-x composer clear
```

The March 12 raw session
`/Users/pc/.codex/sessions/2026/03/12/rollout-2026-03-12T14-57-22-019ce40d-e704-7fe2-9311-3a9adeb4099d.jsonl`
records the user asking for a dedicated Codex-side input clear shortcut and
settling on `Ctrl-X`. That work then went through a clean PR-branch cleanup.

The same May-preserved range next records Talon restoration with an April 16
author date:

```text
7101a98f8053  2026-04-16T14:05:11-07:00  tui: restore Talon file RPC support
```

That is why the May `FORK_SPEC.md` commit inventory reads as if the local fork
begins in March 2026.

### Older preserved fork branch

The repo still has older local backup refs. On
`pc/backup-local-main-2026-02-11`, local commits not in current
`upstream/main` begin with:

```text
3e4e93333b57  2025-09-29T11:19:59-07:00  Add undo/redo and line editing shortcuts to TUI text area
040d1986c28e  2025-09-29T11:20:14-07:00  Stabilize status snapshots against version changes
5c9cb7adc1c1  2025-09-29T11:33:29-07:00  Support meta escape sequences for Option key navigation
bc09d1f4cdb2  2025-09-29T20:09:18-07:00  Add Talon file RPC CLI and state staging support
```

Those commits are fork work by content and by ref ownership: they modify the
Codex TUI, are on Phil backup refs, and are not ancestors of current
`upstream/main`.

So the evidence-backed start boundary is:

- broad preserved local Codex fork work: no later than September 29, 2025,
- the current May pre-merge fork contract stack: March 12, 2026 onward,
- the large current fork expansion documented by the later history summaries:
  May 4 through May 20, 2026.

The next raw-history jump to understand the original fork start should be
September 29 through October 19, 2025, not another April 2026 window.

## Spec reconciliation

This window does not add a new durable fork contract to the current
`FORK_SPEC.md`.

- The April 16 Talon restore is already represented.
- The March Ctrl-X starting point is already represented in the May fork
  inventory.
- The older September/October branch needs its own historical audit before
  deciding whether any old behavior should be added to the current contract or
  only documented as retired history.

## Appendix: raw matching session files

- `/Users/pc/.codex/sessions/2026/04/09/rollout-2026-04-09T09-46-05-019d7322-fada-72b3-8093-e73caed009fd.jsonl`
- `/Users/pc/.codex/sessions/2026/04/09/rollout-2026-04-09T09-48-22-019d7325-1277-7f51-a190-da0e6fadbb4a.jsonl`
- `/Users/pc/.codex/sessions/2026/04/13/rollout-2026-04-13T17-10-10-019d8952-fd92-7770-a0a4-220f016b031a.jsonl`
- `/Users/pc/.codex/sessions/2026/04/16/rollout-2026-04-16T13-19-20-019d97f2-bb7d-7801-a491-9bf2dcd9fd30.jsonl`
- `/Users/pc/.codex/sessions/2026/04/20/rollout-2026-04-20T14-49-33-019dacde-c408-7fa3-9819-bef8a12c5bee.jsonl`
