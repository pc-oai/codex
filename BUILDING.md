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

## Local Disk Space

Local worktrees still grow their own `codex-rs/target` directories. To clear
inactive Cargo output while retaining linked worktrees and archived runnable
builds under `target/local-builds`, use:

```bash
codex-local-prune --apply --min-age-hours 0 --target-free-gb 100
```

The utility skips a worktree while Cargo, rustc, Clippy, or sccache is using
it. A LaunchAgent named `com.pc.codex-local-prune` runs it every 30 minutes:
it normally prunes only targets untouched for 24 hours, but below 40 GiB free
it may prune any inactive target until the laptop reaches 100 GiB free.

Cargo does not currently garbage-collect workspace build artifacts in
`target/`. Its documented
[`[cache] auto-clean-frequency`](https://doc.rust-lang.org/cargo/reference/config.html#cacheauto-clean-frequency)
setting manages downloaded registry and Git dependency data under
`$CARGO_HOME`, not these build directories. A shared `build.target-dir` would
reduce duplicate output but would bring back build locking between concurrent
local Codex sessions, so it is intentionally not configured here.

## Current Remote

- Host: `ec2-user@3.147.77.99`
- SSH key: `~/.ssh/id_ed25519`
- Remote root: `/Users/ec2-user/ci/builds/pc-codex-rust-m4pro`
- Current mirrored checkout: `/Users/ec2-user/ci/builds/pc-codex-rust-m4pro/codex`
- Per-session lanes: `/Users/ec2-user/ci/builds/pc-codex-rust-m4pro/lanes/<lane>`

The remote Mac is intentionally temporary infrastructure. If the host IP, key,
or security group changes, update this file and
`~/src/openai-scripts/codex-mac`.

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
Sync checks file contents and gives transferred files fresh remote timestamps,
so Cargo cannot reuse output compiled from older mirror contents.

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
Commands in the same lane serialize locally. Normal commands wait to sync this
checkout before building, so they do not silently build an older frozen mirror
while another lane is active. When several commands intentionally use an
already-synced frozen snapshot, pass `--no-sync` after the first sync; those
commands may compile at the same time on the M4. The watcher waits while builds
hold the canonical mirror frozen, then catches the mirror up when the build
lock clears.

For manual use outside Codex, pass an explicit lane when you care about cache
reuse or isolation:

```bash
codex-mac --lane tui-repair -- cargo check -p codex-tui -p codex-cli
```

## Iteration Loop

Prefer the remote Mac for build and test feedback during normal Codex Rust
iteration:

1. Edit source locally and run source-only local steps such as `just fmt` when
   required.
2. Run heavy checks, tests, and lint fixes through `codex-mac` while the change
   is still moving.
3. Use `scripts/local-build-codex` once at the end when the result is settled
   and Phil needs a runnable local source build.

Do not use the final local build as an iteration loop. It bumps
`LOCAL_BUILD_NUMBER`, archives a local build slot, and can make later local TUI
test runs rebuild or refresh snapshots that include the local build label.

## Boundaries

- Prefer `codex-mac` for heavy read/write-safe Rust commands: `cargo check`,
  `cargo test`, `cargo nextest`, `just fix -p ...`, and similar validation.
- `codex-mac` defaults remote Cargo commands to `CARGO_INCREMENTAL=0` and
  `RUSTC_WRAPPER=/opt/homebrew/bin/sccache`, with a shared M4-local sccache
  directory under `/Users/ec2-user/ci/builds/pc-codex-rust-m4pro/sccache`.
  Override those env vars explicitly when a session needs different behavior.
- Each lane keeps its own persistent Cargo target directory, and uses Cargo's
  standard target-contained build output rather than overriding
  `CARGO_BUILD_BUILD_DIR`.
  Build-script output can record absolute target paths, so cloning compiled
  output across lanes can produce invalid linker paths. New helper versions
  clear an older seeded lane once before reusing it in this isolated layout.
- The legacy shared seed under
  `/Users/ec2-user/ci/builds/pc-codex-rust-m4pro/seed` is not consumed or
  updated by normal builds. Cross-lane reuse is limited to sccache until a
  path-safe artifact scheme exists.
- Useful operator commands:

  ```bash
  codex-mac --status
  codex-mac --restart-sccache
  codex-mac --prune-lanes 2
  tmux attach -t codex-mac-prune
  codex-mac-htop
  ```
- Keep `scripts/local-build-codex` local unless a session is explicitly working
  on a remote runnable-binary flow. That script mutates
  `codex-rs/tui/src/version.rs` and archives local build slots.
- `codex-mac --sync-back` does not pull built artifacts today. Its source rsync
  excludes `codex-rs/target/`, so a remote `target/debug/codex` or remote
  `target/local-builds` directory will stay on the M4.
- A better remote runnable-binary flow would build the macOS artifact on the M4,
  copy that binary into the local `target/local-builds` archive, and sync the
  matching `LOCAL_BUILD_NUMBER` source change back as one explicit operation.
  Until that exists, keep the one final runnable-binary build local.
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
- 2026-05-22: Added Homebrew to the M4 user's non-interactive zsh path in
  `~/.zshenv` and installed Ghostty's `xterm-ghostty` terminfo entry there.
  Added `codex-mac-htop`, which uses forced SSH terminal allocation (`-tt`) so
  it also works in fish retry loops where bare `ssh ... htop` produces
  `TERM=unknown`.
- 2026-05-22: Remote lane storage reached `1.2T`. `codex-mac --prune-lanes 1
  --prune-target-free-gb 300` now skips active lanes, protects lanes newer than
  the requested age when possible, then removes oldest inactive lanes until
  the M4 reaches the free-space target. A `codex-mac-prune` tmux session runs
  that command every 30 minutes.
- 2026-05-22: Loaded local LaunchAgent `com.pc.codex-local-prune` with a
  30-minute interval. A one-off `codex-local-prune --apply --min-age-hours 0
  --target-free-gb 100` cleanup skipped the actively compiling main target,
  removed three inactive target caches, and raised available local disk space
  from `21.6 GiB` to `69.9 GiB` while retaining archived local builds.
- 2026-05-22: For Codex Rust iteration, keep heavy build and test feedback on
  `codex-mac` and reserve `scripts/local-build-codex` for the final runnable
  local binary. The installed helper currently excludes `codex-rs/target/`
  during sync-back, so a remote final-binary path still needs an explicit
  artifact pull-down flow.
- 2026-05-21: `codex-mac --lane fork-audit-clean --no-sync -- env -u
  CARGO_BUILD_BUILD_DIR cargo test -p codex-exec --no-fail-fast` passed after
  the default remote command failed only in integration cases that launch
  `codex-exec`. With `CARGO_BUILD_BUILD_DIR` set, the runtime binary lookup
  searched `cargo-build/debug/codex-exec` while Cargo left that built binary in
  `cargo-target/debug/codex-exec`; unset the build-dir override for remote
  integration tests that rely on `codex_utils_cargo_bin` until those paths are
  aligned.
- 2026-05-21: For clean-clone verification while another session keeps syncing
  a dirty checkout into the canonical mirror, use a dedicated source mirror as
  well as a dedicated lane, for example `codex-mac --remote-mirror
  /Users/ec2-user/ci/builds/pc-codex-rust-m4pro/mirrors/fork-audit-compact-header
  --lane fork-audit-compact-header-isolated -- cargo test -p codex-tui
  compact_session_header`. A dedicated lane alone still builds whatever source
  is in the shared canonical mirror when the command starts.
- 2026-05-21: A warm dirty lane can retain stale test-profile protocol artifacts
  across app-server API edits and report an impossible missing root re-export
  while `cargo check` from rebuilt dev artifacts passes. When source inspection
  shows the export is present, rerun the proof from a fresh target/build dir (or
  a clean lane) before editing around that error.
- 2026-05-22: A clean terminal-proof build should use a uniquely named
  worktree and an isolated local target when concurrent sessions are active.
  One generic verification worktree was removed during
  `scripts/local-build-codex`, and an attempted shared-target retry inherited
  `RUSTC_WRAPPER=sccache` even though no local `sccache` executable was
  available. Building from `codex-startup-edit-proof-5a7c` with that wrapper
  unset produced archived clean build `v66` in `6m54s`.
- 2026-05-26: During queued-follow-up footer work, local `cargo test` waited
  behind Zed rust-analyzer all-targets checks in the shared local target; move
  focused Rust validation to `codex-mac` immediately when this contention is
  visible. A default remote lane then failed first with stale
  `ThreadSpawnHistory` protocol artifacts and again while linking with stale
  `fork-edit-preview` library paths plus missing `clang_rt.osx`; retry this
  class of failure with a fresh isolated remote lane and report recurrence. A
  fresh lane passed `bottom_pane::footer::tests::footer_snapshots` after its
  cold build, but a separately invoked follow-up test in the same requested
  lane rebuilt broadly; process evidence showed the first command set
  `CARGO_BUILD_BUILD_DIR` and the follow-up did not, while other sessions were
  also syncing the canonical mirror. Combine related filters into one remote
  command or use a dedicated source mirror for stable multi-test evidence, and
  inspect the helper's lane/build-dir handling if repeat warm reuse is needed.
  Once that lane was on its stable target layout, a third focused TUI test
  reused the cache and completed in `0.50s`. Remote `just fix -p codex-tui`
  initially failed because `/opt/homebrew/bin/just` was not installed on the
  M4; installing Homebrew `just` (`1.51.0`) restored the prescribed lint
  command path.
- 2026-05-26: `codex-mac --lane alt-l-native-scrollback-tui -- cargo test -p
  codex-tui redraw_full_scrollback` linked against a stale absolute output
  path from lane `fork-edit-preview` and failed to find `clang_rt.osx`.
  `codex-mac` now keeps Cargo output lane-local, resets lanes once when
  migrating away from the shared seed layout, and no longer publishes failed
  or successful lane output into that unsafe shared seed. It also stops
  setting `CARGO_BUILD_BUILD_DIR`, which previously made tests that launch
  first-party binaries look in a directory where Cargo did not put them.
- 2026-05-26: A follow-up `codex-mac --lane alt-l-native-scrollback-lib --
  cargo test -p codex-tui --lib keymap_setup::tests` ran against a partially
  stale canonical mirror while another lane was active: the feature code was
  present but the latest test/snapshot edits were missing. Normal invocations
  now wait for an exclusive source sync before building; `--no-sync` is the
  explicit opt-in for concurrent builds of a deliberately frozen snapshot.
- 2026-05-26: After the mirror-sync fix, that same keymap test read updated
  snapshots but reused a compiled test binary whose source expectation was
  stale. `rsync -a` had preserved a source timestamp older than the binary
  built during the prior mixed run, so Cargo did not rebuild it. Source syncs
  now use checksums and do not preserve file modification times, ensuring
  transferred source changes invalidate Cargo output.
- 2026-05-26: While validating queue-only footer indication, a later
  `codex-mac` invocation could not start because direct SSH to the M4 host
  `3.147.77.99:22` timed out. Focused local snapshot tests had already passed;
  use local verification only as a declared fallback during this outage and
  re-check M4 reachability before assuming a builder-capacity shortfall.
