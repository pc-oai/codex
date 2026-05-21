# Experimental Codex Build Offload

This is a Phil-specific experimental runbook for keeping heavy Codex Rust
builds off the local 14-inch laptop.

Status: experimental. Expect this to change. When a Codex session uses this
path and learns something concrete, append a dated note under `Learnings` with
the command, result, and any caveat that would help the next session.

## Goal

Make heavy Rust build, check, fix, and test commands feel close to local while
running their expensive work on Phil's AWS M4 Pro Mac.

This is not distributed Cargo. One command still runs on one machine. The win
is that `rustc`, linking, test execution, filesystem churn, thermal load, and
local security-tool overhead move off the laptop.

## Current Remote

- Host: `ec2-user@3.147.77.99`
- SSH key: `~/.ssh/id_ed25519`
- Remote root: `/Users/ec2-user/ci/builds/pc-codex-rust-m4pro`
- Current mirrored checkout: `/Users/ec2-user/ci/builds/pc-codex-rust-m4pro/codex`
- Per-session lanes: `/Users/ec2-user/ci/builds/pc-codex-rust-m4pro/lanes/<lane>`

The remote Mac is intentionally temporary infrastructure. If the host IP, key,
or security group changes, update this file and `~/bin/pc/codex-mac`.

## Use

For heavy Rust commands during Codex work, prefer:

```bash
codex-mac -- cargo check -p codex-tui -p codex-cli
codex-mac -- cargo test -p codex-tui
codex-mac -- just fix -p codex-tui
```

`codex-mac` safety-syncs the current worktree into the canonical M4 mirror when
the mirror is idle, then runs the command from that frozen canonical mirror
while using lane-specific Cargo directories. Pass `--sync-back` only when the
remote command intentionally edits source and those edits should come back.

For the fastest normal loop from the main Codex checkout, keep a foreground
watcher running in another pane:

```bash
cd ~/code/codex
codex-mac --watch
```

That keeps `/Users/ec2-user/ci/builds/pc-codex-rust-m4pro/codex` ready as local
files change. `codex-mac -- <command>` still does a cheap safety rsync before a
command so an immediate build after an edit does not race the watcher debounce.

When Codex invokes shell commands, it injects `CODEX_THREAD_ID`. `codex-mac`
uses that as the lane name, so concurrent Codex sessions get separate remote
Cargo directories instead of serializing on one shared build directory.
Commands in the same lane serialize locally; commands in different lanes can
compile at the same time on the M4. The watcher waits while builds hold the
canonical mirror frozen, then catches the mirror up when the build lock clears.

For manual use outside Codex, pass an explicit lane when you care about cache
reuse or isolation:

```bash
codex-mac --lane tui-repair -- cargo check -p codex-tui -p codex-cli
```

## Boundaries

- Prefer `codex-mac` for heavy read/write-safe Rust commands: `cargo check`,
  `cargo test`, `cargo nextest`, `just fix -p ...`, and similar validation.
- `codex-mac` defaults remote Cargo commands to `CARGO_INCREMENTAL=0` and
  `RUSTC_WRAPPER=/opt/homebrew/bin/sccache`, with a shared M4-local sccache
  directory under `/Users/ec2-user/ci/builds/pc-codex-rust-m4pro/sccache`.
  Override those env vars explicitly when a session needs different behavior.
- New lanes APFS-clone their Cargo dirs from the shared seed under
  `/Users/ec2-user/ci/builds/pc-codex-rust-m4pro/seed`, then merge their Cargo
  dirs back after the command. This is the primary cross-lane warm-cache path;
  do not rely on sccache alone to share Rust artifacts across different lane
  paths.
- Useful operator commands:

  ```bash
  codex-mac --status
  codex-mac --restart-sccache
  codex-mac --prune-lanes 2
  ssh -t -i ~/.ssh/id_ed25519 ec2-user@3.147.77.99 htop
  ```
- Keep `scripts/local-build-codex` local unless a session is explicitly working
  on a remote runnable-binary flow. That script mutates
  `codex-rs/tui/src/version.rs` and archives local build slots.
- Do not assume remote builds are automatically faster for every command. The
  confirmed clean `scripts/local-build-codex` benchmark on 2026-05-21 was:
  local Mac `199.29s`, AWS M4 Pro `163.49s` after Rust toolchain warm-up.
- Do not use this as a substitute for separate worktrees when concurrent tasks
  edit the same files. It isolates Cargo lanes, not source ownership.

## Learnings

- 2026-05-21: Clean `scripts/local-build-codex` on the same Codex snapshot was
  `199.29s` locally and `163.49s` on the AWS M4 Pro after toolchain warm-up.
  The M4 was about 18% faster and kept the heavy work off the laptop.
- 2026-05-21: M1 Ultra was slower for this Codex clean build: `230.72s`
  toolchain-warm versus M4 Pro `163.49s`. Use the M4 for now.
- 2026-05-21: `codex-mac` smoke-tested with
  `CODEX_THREAD_ID=smoke-thread codex-mac -- cargo check -p codex-utils-string`.
  The command passed on the M4 with lane-specific `CARGO_TARGET_DIR` and
  `CARGO_BUILD_BUILD_DIR`.
- 2026-05-21: Switched the experimental shape from per-lane source mirrors to
  one canonical remote mirror plus per-lane Cargo dirs. This matches a
  persistent `codex-mac --watch` rsync from the main checkout and avoids
  copying the full source tree into every Codex thread lane.
- 2026-05-21: `codex-mac --lane fork-edit-preview -- cargo test -p
  codex-tui footer_mode_edit_last_message_snapshot` avoided the laptop's busy
  Cargo target while validating a dirty TUI edit. A cold lane still paid a full
  first `codex-tui` test build and took about four minutes before reaching the
  focused snapshot.
- 2026-05-21: A live `codex-mac -- cargo test -p codex-tui snippet`
  from `codex-rs` still ran from
  `lanes/$CODEX_THREAD_ID/worktree/codex-rs` and started a cold lane build even
  while a `codex-mac --watch` process was running. The focused run reached
  tests after `4m04s` and then failed only because it wrote an expected new TUI
  snapshot. If the watcher is meant to warm command source/cache state for
  Codex sessions, verify that the command path reuses the warmed mirror or
  explain the first-build lane cost here.
- 2026-05-21: During that same live run, the checked-in guidance and the
  installed `~/bin/pc/codex-mac` behavior were out of step. This file says the
  current shape is a canonical remote mirror and that sync-back is opt-in via
  `--sync-back`; the installed script exposed `--no-sync-back`, synced back by
  default, and built from a per-lane `worktree` path. A follow-up run after the
  helper changed rejected `--no-sync-back`, used the default no-sync-back path,
  compiled from the canonical `codex/codex-rs` mirror, and reached the same
  focused TUI test in `32.62s` on the warm lane.
- 2026-05-21: The default sync-back is hazardous during live multi-agent work:
  the same focused test rsynced the lane back with deletion enabled and removed
  this untracked `BUILDING.md` after it had been edited locally while the
  remote command was running. Keep the helper from deleting local post-push
  edits, or make that risk explicit and keep write-back opt-in.
- 2026-05-21: `codex-mac -- cargo check -p codex-app-server -p codex-tui`
  moved a protocol-plus-TUI model-menu validation off the busy local target.
  Focused follow-up tests on the same Codex lane reused the remote cache, but
  concurrent `codex-mac` commands on that lane serialized behind each other.
- 2026-05-21: A lane-worktree snapshot design let watcher syncs continue during
  builds, but Cargo then rebuilt from each lane's different source path. The
  current design instead runs builds from one frozen canonical mirror while the
  watcher waits for shared build locks to clear.
- 2026-05-21: Two clean
  `cargo check -p codex-tui -p codex-cli` runs in lanes
  `concurrency-heavy-a` and `concurrency-heavy-b` started `1.76s` apart and
  finished `3m20s` and `3m19s` later. A remote process check during the run
  showed both Cargo processes live at once with different lane worktrees and
  different `CARGO_TARGET_DIR` / `CARGO_BUILD_BUILD_DIR` paths.
- 2026-05-21: Two commands with the same `CODEX_THREAD_ID=same-lane` did not
  overlap; the second printed `codex-mac: waiting for lane same-lane` and only
  started after the first ended. This is intentional to keep one lane's Cargo
  dirs reliable.
- 2026-05-21: A throwaway cache-path test built
  `cargo check -p codex-utils-string` into a seed Cargo dir at the canonical
  mirror path, APFS-cloned that Cargo dir into a fresh lane, then reran the
  same check from the same canonical source path in `0.23s`. This is why the
  current design keeps source path stable and clones only Cargo dirs per lane.
- 2026-05-21: After priming the shared seed with
  `codex-mac --lane seed-prime --no-sync -- cargo check -p codex-tui -p
  codex-cli`, a fresh seeded lane reran the same focused check in a few seconds.
  Two more fresh seeded lanes, `seed-concurrency-c` and
  `seed-concurrency-d`, started `0.04s` apart and ended at the same timestamp
  after overlapping their checks; the remaining wrapper time was only cache
  publication back into the seed.
- 2026-05-21: Normal `codex-mac -- ...` without `--no-sync` also overlapped
  across lanes once the canonical mirror was frozen. In the proof run,
  `default-concurrency-b` started first, `default-concurrency-a` joined while
  it was still active, and both ran from the canonical `codex/codex-rs` path
  with different Cargo dirs.
- 2026-05-21: Installed `htop 3.5.1` on the M4. Use
  `ssh -t -i ~/.ssh/id_ed25519 ec2-user@3.147.77.99 htop`; Homebrew notes that
  `sudo htop` is needed there to see every process.
