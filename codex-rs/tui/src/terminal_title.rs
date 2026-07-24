//! Terminal-title output helpers for the TUI.
//!
//! This module owns the low-level OSC title write path and the sanitization
//! that happens immediately before we emit it. It is intentionally narrow:
//! callers decide when the title should change and whether an empty title means
//! "leave the old title alone" or "clear the title Codex last wrote".
//! This module does not attempt to read or restore the terminal's previous
//! title because that is not portable across terminals.
//!
//! Sanitization is necessary because title content is assembled from untrusted
//! text sources such as model output, thread names, project paths, and config.
//! Before we place that text inside an OSC sequence, we strip:
//! - control characters that could terminate or reshape the escape sequence
//! - bidi/invisible formatting codepoints that can visually reorder or hide
//!   text (the same family of issues discussed in Trojan Source writeups)
//! - redundant whitespace that would make titles noisy or hard to scan

#[cfg(target_os = "macos")]
use std::ffi::CString;
use std::fmt;
use std::io;
use std::io::IsTerminal;
use std::io::stdout;
#[cfg(target_os = "macos")]
use std::sync::Mutex;
#[cfg(target_os = "macos")]
use std::sync::OnceLock;

use crossterm::Command;
use ratatui::crossterm::execute;

/// Practical upper bound on title length, measured in Rust `char`s.
///
/// Most terminals silently truncate titles beyond a few hundred characters.
/// 240 leaves headroom for the OSC framing bytes while keeping titles
/// readable in tab bars and window managers.
const MAX_TERMINAL_TITLE_CHARS: usize = 240;

/// Outcome of a [`set_terminal_title`] call.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum SetTerminalTitleResult {
    /// A sanitized title was written, or stdout is not a terminal so no write was needed.
    Applied,
    /// Sanitization removed every visible character, so no title was emitted.
    ///
    /// This is distinct from clearing the title. Callers decide whether an
    /// empty post-sanitization value should result in no-op behavior, clearing
    /// the title Codex manages, or some other fallback.
    NoVisibleContent,
}

/// Writes a sanitized OSC window-title sequence to stdout.
///
/// The input is treated as untrusted display text: control characters,
/// invisible formatting characters, and redundant whitespace are removed before
/// the title is emitted. If sanitization removes all visible content, the
/// function returns [`SetTerminalTitleResult::NoVisibleContent`] instead of
/// clearing the title because clearing and restoring are policy decisions for
/// higher-level callers. Mechanically, sanitization collapses whitespace runs
/// to single spaces, drops disallowed codepoints, and bounds the result to
/// [`MAX_TERMINAL_TITLE_CHARS`] visible characters before writing OSC 0.
pub(crate) fn set_terminal_title(title: &str) -> io::Result<SetTerminalTitleResult> {
    if !stdout().is_terminal() {
        return Ok(SetTerminalTitleResult::Applied);
    }

    let title = sanitize_terminal_title(title);
    if title.is_empty() {
        return Ok(SetTerminalTitleResult::NoVisibleContent);
    }

    execute!(stdout(), SetWindowTitle(title))?;
    Ok(SetTerminalTitleResult::Applied)
}

/// Updates the LaunchServices display name shown by Activity Monitor on macOS.
///
/// macOS does not expose a public setter for this name, so resolve the same
/// private LaunchServices functions used by libuv from an Apple-signed system
/// framework. Other platforms do not change their process display name.
pub(crate) fn set_process_title(title: &str) {
    set_process_title_impl(&sanitize_terminal_title(title));
}

#[cfg(not(target_os = "macos"))]
fn set_process_title_impl(_title: &str) {}

#[cfg(target_os = "macos")]
fn set_process_title_impl(title: &str) {
    if title.is_empty() {
        return;
    }

    static PROCESS_TITLE_STATE: OnceLock<Mutex<ProcessTitleState>> = OnceLock::new();
    let state = PROCESS_TITLE_STATE.get_or_init(|| Mutex::new(ProcessTitleState::default()));
    let mut state = state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if state.disabled || state.last_title.as_deref() == Some(title) {
        return;
    }

    let Some(api) = launch_services_api() else {
        state.disabled = true;
        return;
    };
    let Ok(title) = CString::new(title) else {
        tracing::debug!("process display name contains an unexpected NUL");
        state.disabled = true;
        return;
    };

    // SAFETY: all function pointers came from the permanently loaded system
    // framework, and the allocator and UTF-8 encoding match CoreFoundation.
    let display_name =
        unsafe { (api.create_string)(std::ptr::null(), title.as_ptr(), CF_STRING_ENCODING_UTF8) };
    if display_name.is_null() {
        tracing::debug!("failed to create the process display name");
        state.disabled = true;
        return;
    }

    // SAFETY: LaunchServices checked in during initialization. The ASN and
    // display-name key are borrowed; only our newly created string is released.
    let asn = unsafe { (api.current_application_asn)() };
    if asn.is_null() {
        unsafe { (api.release)(display_name) };
        tracing::debug!("LaunchServices did not return a current application ASN");
        state.disabled = true;
        return;
    }

    let status = unsafe {
        (api.set_application_information)(
            -2,
            asn,
            api.display_name_key,
            display_name,
            std::ptr::null_mut(),
        )
    };
    unsafe { (api.release)(display_name) };
    if status == 0 {
        state.last_title = Some(title.to_string_lossy().into_owned());
    } else {
        tracing::debug!(status, "failed to update the LaunchServices display name");
        state.disabled = true;
    }
}

#[cfg(target_os = "macos")]
#[derive(Default)]
struct ProcessTitleState {
    last_title: Option<String>,
    disabled: bool,
}

#[cfg(target_os = "macos")]
type CfTypeRef = *const std::ffi::c_void;
#[cfg(target_os = "macos")]
type CfStringRef = CfTypeRef;
#[cfg(target_os = "macos")]
type CfBundleRef = CfTypeRef;
#[cfg(target_os = "macos")]
type CfDictionaryRef = CfTypeRef;

#[cfg(target_os = "macos")]
const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

#[cfg(target_os = "macos")]
type CreateCfString = unsafe extern "C" fn(CfTypeRef, *const std::ffi::c_char, u32) -> CfStringRef;
#[cfg(target_os = "macos")]
type ReleaseCfType = unsafe extern "C" fn(CfTypeRef);
#[cfg(target_os = "macos")]
type GetMainBundle = unsafe extern "C" fn() -> CfBundleRef;
#[cfg(target_os = "macos")]
type GetBundleInfoDictionary = unsafe extern "C" fn(CfBundleRef) -> CfDictionaryRef;
#[cfg(target_os = "macos")]
type ResetLaunchServicesConnection = unsafe extern "C" fn(u64, *mut std::ffi::c_void);
#[cfg(target_os = "macos")]
type CheckIntoLaunchServices = unsafe extern "C" fn(i32, CfDictionaryRef) -> CfDictionaryRef;
#[cfg(target_os = "macos")]
type GetCurrentApplicationAsn = unsafe extern "C" fn() -> CfTypeRef;
#[cfg(target_os = "macos")]
type SetApplicationInformation =
    unsafe extern "C" fn(i32, CfTypeRef, CfStringRef, CfStringRef, *mut CfDictionaryRef) -> i32;

#[cfg(target_os = "macos")]
struct LaunchServicesApi {
    create_string: CreateCfString,
    release: ReleaseCfType,
    current_application_asn: GetCurrentApplicationAsn,
    set_application_information: SetApplicationInformation,
    display_name_key: CfStringRef,
}

#[cfg(target_os = "macos")]
// SAFETY: the only stored pointer refers to an immutable, borrowed CFString
// exported by an Apple framework whose dlopen handle is never closed.
unsafe impl Send for LaunchServicesApi {}
#[cfg(target_os = "macos")]
// SAFETY: all fields are immutable, and LaunchServices calls are serialized by
// PROCESS_TITLE_STATE's mutex before they can use the borrowed CFString.
unsafe impl Sync for LaunchServicesApi {}

#[cfg(target_os = "macos")]
fn launch_services_api() -> Option<&'static LaunchServicesApi> {
    static API: OnceLock<Option<LaunchServicesApi>> = OnceLock::new();
    API.get_or_init(|| {
        let framework = c"/System/Library/Frameworks/ApplicationServices.framework/Versions/A/ApplicationServices";
        // SAFETY: the framework path is NUL-terminated and points only to an
        // Apple-signed system framework. Keep its handle open permanently so
        // cached function pointers and the display-name key remain valid.
        let handle = unsafe { libc::dlopen(framework.as_ptr(), libc::RTLD_LAZY | libc::RTLD_LOCAL) };
        if handle.is_null() {
            tracing::debug!("failed to load the system ApplicationServices framework");
            return None;
        }

        let create_string = load_launch_services_symbol(handle, b"CFStringCreateWithCString\0")?;
        let release = load_launch_services_symbol(handle, b"CFRelease\0")?;
        let main_bundle = load_launch_services_symbol(handle, b"CFBundleGetMainBundle\0")?;
        let info_dictionary =
            load_launch_services_symbol(handle, b"CFBundleGetInfoDictionary\0")?;
        let reset_connection = load_launch_services_symbol(
            handle,
            b"_LSSetApplicationLaunchServicesServerConnectionStatus\0",
        )?;
        let check_in = load_launch_services_symbol(handle, b"_LSApplicationCheckIn\0")?;
        let current_asn = load_launch_services_symbol(handle, b"_LSGetCurrentApplicationASN\0")?;
        let set_information =
            load_launch_services_symbol(handle, b"_LSSetApplicationInformationItem\0")?;
        let display_name_key =
            load_launch_services_symbol(handle, b"_kLSDisplayNameKey\0")?;

        // SAFETY: each non-null symbol has the matching CoreFoundation or
        // private LaunchServices C signature used by libuv on macOS.
        let create_string = unsafe { std::mem::transmute::<_, CreateCfString>(create_string) };
        let release = unsafe { std::mem::transmute::<_, ReleaseCfType>(release) };
        let main_bundle = unsafe { std::mem::transmute::<_, GetMainBundle>(main_bundle) };
        let info_dictionary =
            unsafe { std::mem::transmute::<_, GetBundleInfoDictionary>(info_dictionary) };
        let reset_connection =
            unsafe { std::mem::transmute::<_, ResetLaunchServicesConnection>(reset_connection) };
        let check_in = unsafe { std::mem::transmute::<_, CheckIntoLaunchServices>(check_in) };
        let current_application_asn =
            unsafe { std::mem::transmute::<_, GetCurrentApplicationAsn>(current_asn) };
        let set_application_information =
            unsafe { std::mem::transmute::<_, SetApplicationInformation>(set_information) };

        // SAFETY: _kLSDisplayNameKey is an exported pointer to an immutable
        // CFStringRef, not a CFStringRef itself, and remains framework-owned.
        let display_name_key = unsafe { *display_name_key.cast::<CfStringRef>() };
        if display_name_key.is_null() {
            tracing::debug!("LaunchServices did not expose a display-name key");
            return None;
        }

        // SAFETY: these exact calls reset LaunchServices and check this CLI
        // process in before its current application ASN is requested.
        unsafe { reset_connection(0, std::ptr::null_mut()) };
        let bundle = unsafe { main_bundle() };
        if bundle.is_null() {
            tracing::debug!("CoreFoundation did not expose a main application bundle");
            return None;
        }
        let info = unsafe { info_dictionary(bundle) };
        if info.is_null() {
            tracing::debug!("CoreFoundation did not expose application bundle information");
            return None;
        }
        unsafe { check_in(-2, info) };

        Some(LaunchServicesApi {
            create_string,
            release,
            current_application_asn,
            set_application_information,
            display_name_key,
        })
    })
    .as_ref()
}

#[cfg(target_os = "macos")]
fn load_launch_services_symbol(
    handle: *mut std::ffi::c_void,
    name: &'static [u8],
) -> Option<*mut std::ffi::c_void> {
    // SAFETY: all callers provide NUL-terminated symbol names, and the
    // ApplicationServices framework handle remains loaded for this process.
    let symbol = unsafe { libc::dlsym(handle, name.as_ptr().cast()) };
    if symbol.is_null() {
        tracing::debug!(symbol = ?name, "required LaunchServices symbol was unavailable");
        None
    } else {
        Some(symbol)
    }
}

/// Clears the current terminal title by writing an empty OSC title payload.
///
/// This clears the visible title; it does not restore whatever title the shell
/// or a previous program may have set before Codex started managing the title.
pub(crate) fn clear_terminal_title() -> io::Result<()> {
    if !stdout().is_terminal() {
        return Ok(());
    }

    execute!(stdout(), SetWindowTitle(String::new()))
}

#[derive(Debug, Clone)]
struct SetWindowTitle(String);

impl Command for SetWindowTitle {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        // Match crossterm's SetTitle command and terminate OSC 0 with BEL.
        // Some terminal title integrations expose the ST terminator in process
        // decorations even though they otherwise accept the title update.
        write!(f, "\x1b]0;{}\x07", self.0)
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Err(std::io::Error::other(
            "tried to execute SetWindowTitle using WinAPI; use ANSI instead",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

/// Normalizes untrusted title text into a single bounded display line.
///
/// This removes terminal control characters, strips invisible/bidi formatting
/// characters, collapses any whitespace run into a single ASCII space, and
/// truncates after [`MAX_TERMINAL_TITLE_CHARS`] emitted characters.
fn sanitize_terminal_title(title: &str) -> String {
    let mut sanitized = String::new();
    let mut chars_written = 0;
    let mut pending_space = false;

    for ch in title.chars() {
        if ch.is_whitespace() {
            // Only set pending if we've already written content; this
            // strips leading whitespace without an extra trim pass.
            pending_space = !sanitized.is_empty();
            continue;
        }

        if is_disallowed_terminal_title_char(ch) {
            continue;
        }

        if pending_space {
            let remaining = MAX_TERMINAL_TITLE_CHARS.saturating_sub(chars_written);
            if remaining > 1 {
                sanitized.push(' ');
                chars_written += 1;
                pending_space = false;
            }
        }

        if chars_written >= MAX_TERMINAL_TITLE_CHARS {
            break;
        }

        sanitized.push(ch);
        chars_written += 1;
    }

    sanitized
}

/// Returns whether `ch` should be dropped from terminal-title output.
///
/// This includes both plain control characters and a curated set of invisible
/// formatting codepoints. The bidi entries here cover the Trojan-Source-style
/// text-reordering controls that can make a title render misleadingly relative
/// to its underlying byte sequence.
fn is_disallowed_terminal_title_char(ch: char) -> bool {
    if ch.is_control() {
        return true;
    }

    // Strip Trojan-Source-related bidi controls plus common non-rendering
    // formatting characters so title text cannot smuggle terminal control
    // semantics or visually misleading content.
    matches!(
        ch,
        '\u{00AD}'
            | '\u{034F}'
            | '\u{061C}'
            | '\u{180E}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{206F}'
            | '\u{FE00}'..='\u{FE0F}'
            | '\u{FEFF}'
            | '\u{FFF9}'..='\u{FFFB}'
            | '\u{1BCA0}'..='\u{1BCA3}'
            | '\u{E0100}'..='\u{E01EF}'
    )
}

#[cfg(test)]
mod tests {
    use super::MAX_TERMINAL_TITLE_CHARS;
    use super::SetWindowTitle;
    use super::sanitize_terminal_title;
    use crossterm::Command;
    use pretty_assertions::assert_eq;

    #[test]
    fn sanitizes_terminal_title() {
        let sanitized =
            sanitize_terminal_title("  Project\t|\nWorking\x1b\x07\u{009D}\u{009C} |  Thread  ");
        assert_eq!(sanitized, "Project | Working | Thread");
    }

    #[test]
    fn strips_invisible_format_chars_from_terminal_title() {
        let sanitized = sanitize_terminal_title(
            "Pro\u{202E}j\u{2066}e\u{200F}c\u{061C}t\u{200B} \u{FEFF}T\u{2060}itle",
        );
        assert_eq!(sanitized, "Project Title");
    }

    #[test]
    fn truncates_terminal_title() {
        let input = "a".repeat(MAX_TERMINAL_TITLE_CHARS + 10);
        let sanitized = sanitize_terminal_title(&input);
        assert_eq!(sanitized.len(), MAX_TERMINAL_TITLE_CHARS);
    }

    #[test]
    fn truncation_prefers_visible_char_over_pending_space() {
        let input = format!("{} b", "a".repeat(MAX_TERMINAL_TITLE_CHARS - 1));
        let sanitized = sanitize_terminal_title(&input);
        assert_eq!(sanitized.len(), MAX_TERMINAL_TITLE_CHARS);
        assert_eq!(sanitized.chars().last(), Some('b'));
    }

    #[test]
    fn writes_osc_title_with_bel_terminator() {
        let mut out = String::new();
        SetWindowTitle("hello".to_string())
            .write_ansi(&mut out)
            .expect("encode terminal title");
        assert_eq!(out, "\x1b]0;hello\x07");
    }
}
