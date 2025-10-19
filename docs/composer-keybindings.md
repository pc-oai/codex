# Composer Keyboard Shortcuts

The table below catalogues every keyboard shortcut handled directly by the TUI composer (`codex-rs/tui/src/bottom_pane/textarea.rs`).

| Shortcut(s) | Action | Standard Terminal? | Source | Notes |
|-------------|--------|--------------------|--------|-------|
| `Ctrl+Z`, `Ctrl+_`, raw `^Z` | Undo | GUI standard | Upstream | Handles terminals that emit control bytes without modifier metadata. |
| `Ctrl+Shift+Z`, `Ctrl+Y` | Redo | GUI standard | Upstream | `Ctrl+Y` is repurposed from readline yank. |
| `Alt+Z`, `Alt+Shift+Z` | Undo | No | Custom (Phil) | Ergonomic Mac bindings added on top of upstream undo support. |
| `Alt+Y`, `Alt+Shift+Y` | Redo | No | Custom (Phil) | Complements custom undo bindings. |
| `Enter`, `Ctrl+J`, `Ctrl+M` | Insert newline | Yes | Upstream | Covers both Enter and C0 newline control codes. |
| Any printable char (with `Shift`) | Insert character | Yes | Upstream | ALT-modified characters are suppressed so meta navigation can trigger instead. |
| `Backspace`, `Ctrl+H` | Delete char left | Yes | Upstream | Classic backspace. |
| `Delete`, `Ctrl+D` | Delete char right | Yes | Upstream | Classic forward delete. |
| `Alt+Backspace`, `Ctrl+W`, `Ctrl+Alt+H` | Delete word left | Yes (`Alt+Backspace`, `Ctrl+W`) | Upstream + Custom | `Ctrl+Alt+H` is an extra ergonomics chord. |
| `Alt+Delete`, `Alt+D` | Delete word right | Yes (`Alt+Delete`) | Upstream + Custom | `Alt+D` recently added to mirror readline. |
| `Ctrl+Shift+F`, `Alt+Shift+F` | Delete word right | No | Custom | Safety net for terminals that report extra modifiers. |
| `Ctrl+Backspace` | Delete entire current line | No | Custom | Convenience binding. |
| `Ctrl+Shift+D` | Kill to logical end of line | Partial | Custom | Equivalent to Emacs `Ctrl+K` but leaves newline at EOL. |
| `Alt+Shift+Backspace` | Kill to wrapped line start | No | Custom | Visual-line aware. |
| `Alt+Shift+Delete`, `Alt+Shift+D` | Kill to wrapped line end | No | Custom | Visual-line aware. |
| `Ctrl+U` | Kill to beginning of line | Yes | Upstream | Readline behavior. |
| `Ctrl+K` | Kill to end of line | Yes | Upstream | Readline behavior. |
| Arrow Left/Right, `Ctrl+B`, `Ctrl+F` | Move cursor left/right | Yes | Upstream | Includes fallback for raw control bytes. |
| `Alt+Left/Right`, `Ctrl+Left/Right`, `Alt+B`, `Alt+F` | Word-wise left/right | Yes | Upstream | Works with Option-arrow escape sequences. |
| Arrow Up/Down | Move cursor up/down | Yes | Upstream | Standard vertical movement. |
| `Home`, `End` | Move to line boundaries | Yes | Upstream | Moves to line BOL/EOL. |
| `Ctrl+Home`, `Ctrl+End`, `Alt+Home`, `Alt+End` | Move to start/end of buffer | Yes | Upstream | Jump to absolute buffer boundaries. |
| `Ctrl+A`, `Ctrl+E` | Move to line start/end | Yes | Upstream | Readline behavior. |
| `Esc` sequences (`[C`, `[D`, `[F`, `[H`, etc.) | Meta navigation | Yes | Upstream | Supports terminals that send ESC-prefixed sequences instead of ALT modifiers. |
| `Esc` + `b`/`f`/`d`/`u`/`k` | Meta navigation | Yes | Upstream | Handled via meta sequence buffer when Option sends ESC prefix. |

## Standard Terminal Shortcuts Coverage

| Standard Shortcut | Typical Behavior | Composer Status | Notes |
|-------------------|------------------|-----------------|-------|
| `Ctrl+A` / `Ctrl+E` | Line start / end | Supported | Maps to `move_cursor_to_beginning/end_of_line`. |
| `Ctrl+B` / `Ctrl+F` | Char left / right | Supported | Also catches raw control byte fallbacks. |
| `Ctrl+P` / `Ctrl+N` | Previous / next line | Missing | Use arrow keys or add via Karabiner remap. |
| `Ctrl+H` / `Backspace` | Delete char left | Supported | — |
| `Ctrl+D` | Delete char right | Supported | — |
| `Ctrl+W` | Delete word left | Supported | — |
| `Ctrl+U` / `Ctrl+K` | Kill to BOL / EOL | Supported | — |
| `Ctrl+Y` | Yank (paste last kill) | Rebound (Redo) | Conflict with redo requirement. |
| `Ctrl+L` | Clear screen | Missing | No equivalent in composer. |
| `Ctrl+T` | Transpose characters | Missing | Not implemented. |
| `Ctrl+V` | Insert next char literally | Missing | Meta suppression blocks literal mode. |
| `Alt+B` / `Alt+F` | Word left / right | Supported | Works with Option key or ESC prefix. |
| `Alt+D` | Delete word right | Supported | Newly added for parity. |
| `Alt+Backspace` | Delete word left | Supported | — |
| `Alt+T` | Transpose words | Missing | — |
| `Alt+U` / `Alt+L` / `Alt+C` | Uppercase / lowercase / capitalize word | Missing | — |
| `Ctrl+_` | Undo | Supported | Shares path with `Ctrl+Z`. |
| `Alt+Y` | Yank-pop | Missing (Redo) | Currently mapped to redo. |

## Missing or Reassigned Shortcuts

Shortcuts you might map externally (e.g., via Karabiner Elements) to existing bindings or request for native support:

- `Ctrl+P` / `Ctrl+N` → map to `Up` / `Down` or add explicit handler.
- `Ctrl+L`, `Ctrl+T`, `Ctrl+V` → no composer equivalents today.
- Word transformation/emacs variants (`Alt+T/U/L/C`, `Alt+Y`) → unimplemented.
- `Ctrl+Y` currently performs redo; yank behavior would need an alternative chord.

