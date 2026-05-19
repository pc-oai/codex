use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use crate::app_server_session::AppServerSession;
use crate::exec_command::relativize_to_home;
use crate::key_hint;
use crate::legacy_core::config::Config;
use crate::session_resume::resolve_session_thread_id;
use crate::text_formatting::truncate_text;
use crate::tui::FrameRequester;
use crate::tui::Tui;
use crate::tui::TuiEvent;
use chrono::DateTime;
use chrono::Utc;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadListCwdFilter;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadSortKey;
use codex_app_server_protocol::ThreadSourceKind;
use codex_app_server_protocol::ThreadStatus;
use codex_protocol::ThreadId;
use codex_utils_path as path_utils;
use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::style::Stylize as _;
use ratatui::text::Line;
use ratatui::text::Span;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tracing::warn;
use unicode_width::UnicodeWidthStr;

const PAGE_SIZE: usize = 25;
const LOAD_NEAR_THRESHOLD: usize = 5;
const SESSION_ID_SUFFIX_LEN: usize = 8;

#[derive(Debug, Clone)]
pub struct SessionTarget {
    pub path: Option<PathBuf>,
    pub thread_id: ThreadId,
}

impl SessionTarget {
    pub fn display_label(&self) -> String {
        self.path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| format!("thread {}", self.thread_id))
    }
}

#[derive(Debug, Clone)]
pub enum SessionSelection {
    StartFresh,
    Resume(SessionTarget),
    Fork(SessionTarget),
    Exit,
}

#[derive(Clone, Copy, Debug)]
pub enum SessionPickerAction {
    Resume,
    Fork,
}

#[derive(Clone, Copy, Debug)]
pub enum SessionPickerRuntime {
    Local,
    Remote,
}

impl SessionPickerAction {
    fn title(self) -> &'static str {
        match self {
            SessionPickerAction::Resume => "Resume a previous session",
            SessionPickerAction::Fork => "Fork a previous session",
        }
    }

    fn action_label(self) -> &'static str {
        match self {
            SessionPickerAction::Resume => "resume",
            SessionPickerAction::Fork => "fork",
        }
    }

    fn selection(self, path: Option<PathBuf>, thread_id: ThreadId) -> SessionSelection {
        let target_session = SessionTarget { path, thread_id };
        match self {
            SessionPickerAction::Resume => SessionSelection::Resume(target_session),
            SessionPickerAction::Fork => SessionSelection::Fork(target_session),
        }
    }
}

#[derive(Clone)]
struct PageLoadRequest {
    cursor: Option<PageCursor>,
    request_token: usize,
    search_token: Option<usize>,
    cwd_filter: Option<PathBuf>,
    purpose: LoadPurpose,
    provider_filter: ProviderFilter,
    sort_key: ThreadSortKey,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LoadPurpose {
    Active,
    WarmAllDirectories,
}

#[derive(Clone)]
enum ProviderFilter {
    Any,
    MatchDefault(String),
}

type PageLoader = Arc<dyn Fn(PageLoadRequest) + Send + Sync>;

enum BackgroundEvent {
    PageLoaded {
        request_token: usize,
        search_token: Option<usize>,
        purpose: LoadPurpose,
        page: std::io::Result<PickerPage>,
    },
}

#[derive(Clone)]
enum PageCursor {
    AppServer(String),
}

struct PickerPage {
    rows: Vec<Row>,
    next_cursor: Option<PageCursor>,
    num_scanned_files: usize,
    reached_scan_cap: bool,
}

struct WarmAllDirectoriesCache {
    page: PickerPage,
}

/// Interactive session picker that lists app-server threads with simple search
/// and pagination.
///
/// The picker displays sessions in a table with timestamp columns (created/updated),
/// a session-id suffix, git branch, working directory, and conversation preview. Users can toggle
/// between sorting by creation time and last-updated time using the Tab key.
///
/// Sessions are loaded on-demand via cursor-based pagination. The backend
/// `thread/list` API returns pages ordered by the selected sort key, and the
/// picker deduplicates across pages to handle overlapping windows when new
/// sessions appear during pagination.
///
/// Filtering happens in two layers:
/// 1. Provider, source, and eligible working-directory filtering at the backend.
/// 2. Typed search filtering over loaded rows in the picker.
pub async fn run_resume_picker_with_app_server(
    tui: &mut Tui,
    config: &Config,
    show_all: bool,
    include_non_interactive: bool,
    app_server: AppServerSession,
) -> Result<SessionSelection> {
    let (bg_tx, bg_rx) = mpsc::unbounded_channel();
    let is_remote = app_server.is_remote();
    let scope_cwd_filter = picker_cwd_filter(
        config.cwd.as_path(),
        /*show_all*/ false,
        is_remote,
        app_server.remote_cwd_override(),
    );
    run_session_picker_with_loader(
        tui,
        config,
        show_all,
        scope_cwd_filter,
        SessionPickerAction::Resume,
        is_remote,
        spawn_app_server_page_loader(app_server, include_non_interactive, bg_tx),
        bg_rx,
    )
    .await
}

pub async fn run_fork_picker_with_app_server(
    tui: &mut Tui,
    config: &Config,
    show_all: bool,
    app_server: AppServerSession,
) -> Result<SessionSelection> {
    let (bg_tx, bg_rx) = mpsc::unbounded_channel();
    let is_remote = app_server.is_remote();
    let scope_cwd_filter = picker_cwd_filter(
        config.cwd.as_path(),
        /*show_all*/ false,
        is_remote,
        app_server.remote_cwd_override(),
    );
    run_session_picker_with_loader(
        tui,
        config,
        show_all,
        scope_cwd_filter,
        SessionPickerAction::Fork,
        is_remote,
        spawn_app_server_page_loader(app_server, /*include_non_interactive*/ false, bg_tx),
        bg_rx,
    )
    .await
}

/// Runs a fixed picker over sessions that already matched a direct CLI lookup.
///
/// This is used when a short session-id fragment resolves to more than one
/// candidate. The table stays sorted in the order supplied by the caller and
/// shows all matching directories so the narrowed result set remains legible.
pub async fn run_session_match_picker(
    tui: &mut Tui,
    action: SessionPickerAction,
    id_fragment: &str,
    threads: Vec<Thread>,
    runtime: SessionPickerRuntime,
) -> Result<SessionSelection> {
    let rows = rows_from_app_server_threads(threads, runtime).await;
    let num_scanned_files = rows.len();
    let (_bg_tx, bg_rx) = mpsc::unbounded_channel();
    let alt = AltScreenGuard::enter(tui);
    let mut state = PickerState::new(
        alt.tui.frame_requester(),
        Arc::new(|_: PageLoadRequest| {}),
        ProviderFilter::Any,
        /*show_all*/ true,
        /*scope_cwd_filter*/ None,
        action,
    );
    state.relative_time_reference = Some(Utc::now());
    state.query = id_fragment.to_string();
    state.ingest_page(PickerPage {
        rows,
        next_cursor: None,
        num_scanned_files,
        reached_scan_cap: true,
    });
    state.request_frame();

    run_session_picker_loop(alt, state, bg_rx).await
}

async fn run_session_picker_with_loader(
    tui: &mut Tui,
    config: &Config,
    show_all: bool,
    scope_cwd_filter: Option<PathBuf>,
    action: SessionPickerAction,
    is_remote: bool,
    page_loader: PageLoader,
    bg_rx: mpsc::UnboundedReceiver<BackgroundEvent>,
) -> Result<SessionSelection> {
    let alt = AltScreenGuard::enter(tui);
    let provider_filter = if is_remote {
        ProviderFilter::Any
    } else {
        ProviderFilter::MatchDefault(config.model_provider_id.to_string())
    };
    // Remote sessions live in the server's filesystem namespace, so the client
    // process cwd is not a meaningful row filter. Local cwd filtering and explicit
    // remote --cd filtering are handled server-side in thread/list.
    let mut state = PickerState::new(
        alt.tui.frame_requester(),
        page_loader,
        provider_filter,
        show_all,
        scope_cwd_filter,
        action,
    );
    state.start_initial_load();
    state.request_frame();

    run_session_picker_loop(alt, state, bg_rx).await
}

async fn run_session_picker_loop(
    alt: AltScreenGuard<'_>,
    mut state: PickerState,
    bg_rx: mpsc::UnboundedReceiver<BackgroundEvent>,
) -> Result<SessionSelection> {
    let mut tui_events = alt.tui.event_stream().fuse();
    let mut background_events = UnboundedReceiverStream::new(bg_rx).fuse();

    loop {
        tokio::select! {
            Some(ev) = tui_events.next() => {
                match ev {
                    TuiEvent::Key(key) => {
                        if matches!(key.kind, KeyEventKind::Release) {
                            continue;
                        }
                        if let Some(sel) = state.handle_key(key).await? {
                            return Ok(sel);
                        }
                    }
                    TuiEvent::Draw | TuiEvent::Resize => {
                        if let Ok(size) = alt.tui.terminal.size() {
                            let list_height = size.height.saturating_sub(4) as usize;
                            state.update_view_rows(list_height);
                            state.ensure_minimum_rows_for_view(list_height);
                        }
                        draw_picker(alt.tui, &state)?;
                    }
                    _ => {}
                }
            }
            Some(event) = background_events.next() => {
                state.handle_background_event(event).await?;
            }
            else => break,
        }
    }

    // Fallback – treat as cancel/new
    Ok(SessionSelection::StartFresh)
}

fn picker_cwd_filter(
    config_cwd: &Path,
    show_all: bool,
    is_remote: bool,
    remote_cwd_override: Option<&Path>,
) -> Option<PathBuf> {
    if show_all {
        None
    } else if is_remote {
        remote_cwd_override.map(Path::to_path_buf)
    } else {
        Some(config_cwd.to_path_buf())
    }
}

fn spawn_app_server_page_loader(
    app_server: AppServerSession,
    include_non_interactive: bool,
    bg_tx: mpsc::UnboundedSender<BackgroundEvent>,
) -> PageLoader {
    let (request_tx, mut request_rx) = mpsc::unbounded_channel::<PageLoadRequest>();

    tokio::spawn(async move {
        let mut app_server = app_server;
        while let Some(request) = request_rx.recv().await {
            let cursor = request.cursor.map(|PageCursor::AppServer(cursor)| cursor);
            let page = load_app_server_page(
                &mut app_server,
                cursor,
                request.cwd_filter.as_deref(),
                request.provider_filter,
                request.sort_key,
                include_non_interactive,
            )
            .await;
            let _ = bg_tx.send(BackgroundEvent::PageLoaded {
                request_token: request.request_token,
                search_token: request.search_token,
                purpose: request.purpose,
                page,
            });
        }
        if let Err(err) = app_server.shutdown().await {
            warn!(%err, "Failed to shut down app-server picker session");
        }
    });

    Arc::new(move |request: PageLoadRequest| {
        let _ = request_tx.send(request);
    })
}

/// Returns the human-readable column header for the given sort key.
fn sort_key_label(sort_key: ThreadSortKey) -> &'static str {
    match sort_key {
        ThreadSortKey::CreatedAt => "Created",
        ThreadSortKey::UpdatedAt => "Updated",
    }
}

const CREATED_COLUMN_LABEL: &str = "Created";
const UPDATED_COLUMN_LABEL: &str = "Updated";

/// RAII guard that ensures we leave the alt-screen on scope exit.
struct AltScreenGuard<'a> {
    tui: &'a mut Tui,
}

impl<'a> AltScreenGuard<'a> {
    fn enter(tui: &'a mut Tui) -> Self {
        let _ = tui.enter_alt_screen();
        Self { tui }
    }
}

impl Drop for AltScreenGuard<'_> {
    fn drop(&mut self) {
        let _ = self.tui.leave_alt_screen();
    }
}

struct PickerState {
    requester: FrameRequester,
    relative_time_reference: Option<DateTime<Utc>>,
    pagination: PaginationState,
    all_rows: Vec<Row>,
    filtered_rows: Vec<Row>,
    seen_rows: HashSet<SeenRowKey>,
    selected: usize,
    scroll_top: usize,
    query: String,
    search_state: SearchState,
    next_request_token: usize,
    next_search_token: usize,
    page_loader: PageLoader,
    view_rows: Option<usize>,
    provider_filter: ProviderFilter,
    show_all: bool,
    custom_titles_only: bool,
    scope_cwd_filter: Option<PathBuf>,
    warm_all_directories: Option<WarmAllDirectoriesCache>,
    action: SessionPickerAction,
    sort_key: ThreadSortKey,
    inline_error: Option<String>,
}

struct PaginationState {
    next_cursor: Option<PageCursor>,
    num_scanned_files: usize,
    reached_scan_cap: bool,
    loading: LoadingState,
}

#[derive(Clone, Copy, Debug)]
enum LoadingState {
    Idle,
    Pending(PendingLoad),
}

#[derive(Clone, Copy, Debug)]
struct PendingLoad {
    request_token: usize,
    search_token: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
enum SearchState {
    Idle,
    Active { token: usize },
}

enum LoadTrigger {
    Scroll,
    Search { token: usize },
}

impl LoadingState {
    fn is_pending(&self) -> bool {
        matches!(self, LoadingState::Pending(_))
    }
}

async fn load_app_server_page(
    app_server: &mut AppServerSession,
    cursor: Option<String>,
    cwd_filter: Option<&Path>,
    provider_filter: ProviderFilter,
    sort_key: ThreadSortKey,
    include_non_interactive: bool,
) -> std::io::Result<PickerPage> {
    let runtime = if app_server.is_remote() {
        SessionPickerRuntime::Remote
    } else {
        SessionPickerRuntime::Local
    };
    let response = app_server
        .thread_list(thread_list_params(
            cursor,
            cwd_filter,
            provider_filter,
            sort_key,
            include_non_interactive,
        ))
        .await
        .map_err(std::io::Error::other)?;
    let num_scanned_files = response.data.len();

    Ok(PickerPage {
        rows: rows_from_app_server_threads(response.data, runtime).await,
        next_cursor: response.next_cursor.map(PageCursor::AppServer),
        num_scanned_files,
        reached_scan_cap: false,
    })
}

async fn rows_from_app_server_threads(
    threads: Vec<Thread>,
    runtime: SessionPickerRuntime,
) -> Vec<Row> {
    let mut rows = Vec::with_capacity(threads.len());
    for thread in threads {
        let open_state = match runtime {
            SessionPickerRuntime::Local => {
                if crate::talon::session_command_socket_is_live(thread.id.as_str()).await {
                    SessionOpenState::Open
                } else {
                    SessionOpenState::Closed
                }
            }
            SessionPickerRuntime::Remote => {
                if matches!(&thread.status, ThreadStatus::NotLoaded) {
                    SessionOpenState::Closed
                } else {
                    SessionOpenState::Open
                }
            }
        };
        if let Some(row) = row_from_app_server_thread(thread, open_state) {
            rows.push(row);
        }
    }
    rows
}

impl SearchState {
    fn active_token(&self) -> Option<usize> {
        match self {
            SearchState::Idle => None,
            SearchState::Active { token } => Some(*token),
        }
    }

    fn is_active(&self) -> bool {
        self.active_token().is_some()
    }
}

#[derive(Clone)]
struct Row {
    path: Option<PathBuf>,
    preview: String,
    thread_id: Option<ThreadId>,
    thread_name: Option<String>,
    user_message_count: i64,
    created_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,
    cwd: Option<PathBuf>,
    git_branch: Option<String>,
    open_state: SessionOpenState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SessionOpenState {
    Open,
    Closed,
}

impl SessionOpenState {
    fn label(self) -> &'static str {
        match self {
            SessionOpenState::Open => "yes",
            SessionOpenState::Closed => "no",
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum SeenRowKey {
    Path(PathBuf),
    Thread(ThreadId),
}

impl Row {
    fn seen_key(&self) -> Option<SeenRowKey> {
        if let Some(path) = self.path.clone() {
            return Some(SeenRowKey::Path(path));
        }
        self.thread_id.map(SeenRowKey::Thread)
    }

    fn display_preview(&self) -> &str {
        self.thread_name.as_deref().unwrap_or(&self.preview)
    }

    fn has_custom_title(&self) -> bool {
        self.thread_name.is_some()
    }

    fn session_id_suffix(&self) -> String {
        self.thread_id
            .as_ref()
            .map(|thread_id| {
                let suffix = thread_id
                    .to_string()
                    .chars()
                    .rev()
                    .take(SESSION_ID_SUFFIX_LEN)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect::<String>();
                format!("…{suffix}")
            })
            .unwrap_or_else(|| "-".to_string())
    }

    fn matches_query(&self, query: &str) -> bool {
        if self.preview.to_lowercase().contains(query) {
            return true;
        }
        if let Some(thread_name) = self.thread_name.as_ref()
            && thread_name.to_lowercase().contains(query)
        {
            return true;
        }
        if let Some(thread_id) = self.thread_id.as_ref() {
            let thread_id = thread_id.to_string().to_lowercase();
            if thread_id.contains(query) {
                return true;
            }
            let normalized_thread_id = thread_id.replace('-', "");
            let normalized_query = query.replace('-', "");
            if !normalized_query.is_empty() && normalized_thread_id.contains(&normalized_query) {
                return true;
            }
        }
        false
    }
}

impl PickerState {
    fn new(
        requester: FrameRequester,
        page_loader: PageLoader,
        provider_filter: ProviderFilter,
        show_all: bool,
        scope_cwd_filter: Option<PathBuf>,
        action: SessionPickerAction,
    ) -> Self {
        Self {
            requester,
            relative_time_reference: None,
            pagination: PaginationState {
                next_cursor: None,
                num_scanned_files: 0,
                reached_scan_cap: false,
                loading: LoadingState::Idle,
            },
            all_rows: Vec::new(),
            filtered_rows: Vec::new(),
            seen_rows: HashSet::new(),
            selected: 0,
            scroll_top: 0,
            query: String::new(),
            search_state: SearchState::Idle,
            next_request_token: 0,
            next_search_token: 0,
            page_loader,
            view_rows: None,
            provider_filter,
            show_all,
            custom_titles_only: false,
            scope_cwd_filter,
            warm_all_directories: None,
            action,
            sort_key: ThreadSortKey::UpdatedAt,
            inline_error: None,
        }
    }

    fn request_frame(&self) {
        self.requester.schedule_frame();
    }

    async fn handle_key(&mut self, key: KeyEvent) -> Result<Option<SessionSelection>> {
        self.inline_error = None;
        match key {
            KeyEvent {
                code: KeyCode::Esc, ..
            } => return Ok(Some(SessionSelection::StartFresh)),
            KeyEvent {
                code: KeyCode::Char('c' | 'q' | 'x'),
                modifiers,
                ..
            } if modifiers.contains(KeyModifiers::CONTROL) => {
                return Ok(Some(SessionSelection::Exit));
            }
            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => {
                if let Some(row) = self.filtered_rows.get(self.selected) {
                    let path = row.path.clone();
                    let thread_id = match row.thread_id {
                        Some(thread_id) => Some(thread_id),
                        None => match path.as_ref() {
                            Some(path) => {
                                resolve_session_thread_id(path.as_path(), /*id_str_if_uuid*/ None)
                                    .await
                            }
                            None => None,
                        },
                    };
                    if let Some(thread_id) = thread_id {
                        return Ok(Some(self.action.selection(path, thread_id)));
                    }
                    self.inline_error = Some(match path {
                        Some(path) => {
                            format!("Failed to read session metadata from {}", path.display())
                        }
                        None => {
                            String::from("Failed to read session metadata from selected session")
                        }
                    });
                    self.request_frame();
                }
            }
            KeyEvent {
                code: KeyCode::Up, ..
            }
            | KeyEvent {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('\u{0010}'),
                modifiers: KeyModifiers::NONE,
                ..
            } /* ^P */ => {
                if self.selected > 0 {
                    self.selected -= 1;
                    self.ensure_selected_visible();
                }
                self.request_frame();
            }
            KeyEvent {
                code: KeyCode::Down,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('\u{000e}'),
                modifiers: KeyModifiers::NONE,
                ..
            } /* ^N */ => {
                if self.selected + 1 < self.filtered_rows.len() {
                    self.selected += 1;
                    self.ensure_selected_visible();
                }
                self.maybe_load_more_for_scroll();
                self.request_frame();
            }
            KeyEvent {
                code: KeyCode::PageUp,
                ..
            } => {
                let step = self.view_rows.unwrap_or(10).max(1);
                if self.selected > 0 {
                    self.selected = self.selected.saturating_sub(step);
                    self.ensure_selected_visible();
                    self.request_frame();
                }
            }
            KeyEvent {
                code: KeyCode::PageDown,
                ..
            } => {
                if !self.filtered_rows.is_empty() {
                    let step = self.view_rows.unwrap_or(10).max(1);
                    let max_index = self.filtered_rows.len().saturating_sub(1);
                    self.selected = (self.selected + step).min(max_index);
                    self.ensure_selected_visible();
                    self.maybe_load_more_for_scroll();
                    self.request_frame();
                }
            }
            KeyEvent {
                code: KeyCode::Tab, ..
            } => {
                self.toggle_sort_key();
                self.request_frame();
            }
            KeyEvent {
                code: KeyCode::Char('t'),
                modifiers,
                ..
            } if modifiers.contains(KeyModifiers::CONTROL) => {
                self.toggle_custom_titles_only();
            }
            KeyEvent {
                code: KeyCode::Char('a'),
                modifiers,
                ..
            } if modifiers.contains(KeyModifiers::CONTROL) => {
                self.toggle_all_directories();
            }
            KeyEvent {
                code: KeyCode::Backspace,
                ..
            } => {
                let mut new_query = self.query.clone();
                new_query.pop();
                self.set_query(new_query);
            }
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers,
                ..
            } => {
                // basic text input for search
                if !modifiers.contains(KeyModifiers::CONTROL)
                    && !modifiers.contains(KeyModifiers::ALT)
                {
                    let mut new_query = self.query.clone();
                    new_query.push(c);
                    self.set_query(new_query);
                }
            }
            _ => {}
        }
        Ok(None)
    }

    fn start_initial_load(&mut self) {
        self.relative_time_reference = Some(Utc::now());
        self.reset_pagination();
        self.all_rows.clear();
        self.filtered_rows.clear();
        self.seen_rows.clear();
        self.selected = 0;

        let search_token = if self.query.is_empty() {
            self.search_state = SearchState::Idle;
            None
        } else {
            let token = self.allocate_search_token();
            self.search_state = SearchState::Active { token };
            Some(token)
        };

        let request_token = self.allocate_request_token();
        self.pagination.loading = LoadingState::Pending(PendingLoad {
            request_token,
            search_token,
        });
        self.request_frame();

        (self.page_loader)(PageLoadRequest {
            cursor: None,
            request_token,
            search_token,
            cwd_filter: self.current_cwd_filter(),
            purpose: LoadPurpose::Active,
            provider_filter: self.provider_filter.clone(),
            sort_key: self.sort_key,
        });

        if !self.show_all && self.scope_cwd_filter.is_some() {
            let request_token = self.allocate_request_token();
            (self.page_loader)(PageLoadRequest {
                cursor: None,
                request_token,
                search_token: None,
                cwd_filter: None,
                purpose: LoadPurpose::WarmAllDirectories,
                provider_filter: self.provider_filter.clone(),
                sort_key: self.sort_key,
            });
        }
    }

    async fn handle_background_event(&mut self, event: BackgroundEvent) -> Result<()> {
        match event {
            BackgroundEvent::PageLoaded {
                request_token,
                search_token,
                purpose,
                page,
            } => {
                if purpose == LoadPurpose::WarmAllDirectories {
                    if let Ok(page) = page {
                        self.warm_all_directories = Some(WarmAllDirectoriesCache { page });
                    }
                    return Ok(());
                }
                let pending = match self.pagination.loading {
                    LoadingState::Pending(pending) => pending,
                    LoadingState::Idle => return Ok(()),
                };
                if pending.request_token != request_token {
                    return Ok(());
                }
                self.pagination.loading = LoadingState::Idle;
                let page = page.map_err(color_eyre::Report::from)?;
                self.ingest_page(page);
                let completed_token = pending.search_token.or(search_token);
                self.continue_search_if_token_matches(completed_token);
            }
        }
        Ok(())
    }

    fn reset_pagination(&mut self) {
        self.pagination.next_cursor = None;
        self.pagination.num_scanned_files = 0;
        self.pagination.reached_scan_cap = false;
        self.pagination.loading = LoadingState::Idle;
    }

    fn ingest_page(&mut self, page: PickerPage) {
        if let Some(cursor) = page.next_cursor.clone() {
            self.pagination.next_cursor = Some(cursor);
        } else {
            self.pagination.next_cursor = None;
        }
        self.pagination.num_scanned_files = self
            .pagination
            .num_scanned_files
            .saturating_add(page.num_scanned_files);
        if page.reached_scan_cap {
            self.pagination.reached_scan_cap = true;
        }

        for row in page.rows {
            if let Some(seen_key) = row.seen_key() {
                if self.seen_rows.insert(seen_key) {
                    self.all_rows.push(row);
                }
            } else {
                self.all_rows.push(row);
            }
        }

        self.apply_filter();
    }

    fn apply_filter(&mut self) {
        let base_iter = self
            .all_rows
            .iter()
            .filter(|row| self.row_matches_filter(row));
        if self.query.is_empty() {
            self.filtered_rows = base_iter.cloned().collect();
        } else {
            let q = self.query.to_lowercase();
            self.filtered_rows = base_iter.filter(|r| r.matches_query(&q)).cloned().collect();
        }
        if self.selected >= self.filtered_rows.len() {
            self.selected = self.filtered_rows.len().saturating_sub(1);
        }
        if self.filtered_rows.is_empty() {
            self.scroll_top = 0;
        }
        self.ensure_selected_visible();
        self.request_frame();
    }

    fn row_matches_filter(&self, row: &Row) -> bool {
        if self.custom_titles_only && !row.has_custom_title() {
            return false;
        }
        if self.show_all {
            return true;
        }
        let Some(filter_cwd) = self.scope_cwd_filter.as_ref() else {
            return true;
        };
        let Some(row_cwd) = row.cwd.as_ref() else {
            return false;
        };
        paths_match(row_cwd, filter_cwd)
    }

    fn set_query(&mut self, new_query: String) {
        if self.query == new_query {
            return;
        }
        self.query = new_query;
        self.selected = 0;
        self.apply_filter();
        if self.query.is_empty() {
            self.search_state = SearchState::Idle;
            return;
        }
        if !self.filtered_rows.is_empty() {
            self.search_state = SearchState::Idle;
            return;
        }
        if self.pagination.reached_scan_cap || self.pagination.next_cursor.is_none() {
            self.search_state = SearchState::Idle;
            return;
        }
        let token = self.allocate_search_token();
        self.search_state = SearchState::Active { token };
        self.load_more_if_needed(LoadTrigger::Search { token });
    }

    fn toggle_custom_titles_only(&mut self) {
        self.custom_titles_only = !self.custom_titles_only;
        self.selected = 0;
        self.apply_filter();
        if !self.custom_titles_only || !self.filtered_rows.is_empty() {
            self.search_state = SearchState::Idle;
            return;
        }
        if self.pagination.reached_scan_cap || self.pagination.next_cursor.is_none() {
            self.search_state = SearchState::Idle;
            return;
        }
        let token = self.allocate_search_token();
        self.search_state = SearchState::Active { token };
        self.load_more_if_needed(LoadTrigger::Search { token });
    }

    fn toggle_all_directories(&mut self) {
        self.show_all = !self.show_all;
        if self.show_all
            && let Some(cache) = self.warm_all_directories.take()
        {
            self.reset_pagination();
            self.all_rows.clear();
            self.filtered_rows.clear();
            self.seen_rows.clear();
            self.selected = 0;
            self.ingest_page(cache.page);
            return;
        }
        self.start_initial_load();
    }

    fn current_cwd_filter(&self) -> Option<PathBuf> {
        if self.show_all {
            None
        } else {
            self.scope_cwd_filter.clone()
        }
    }

    fn continue_search_if_needed(&mut self) {
        let Some(token) = self.search_state.active_token() else {
            return;
        };
        if !self.filtered_rows.is_empty() {
            self.search_state = SearchState::Idle;
            return;
        }
        if self.pagination.reached_scan_cap || self.pagination.next_cursor.is_none() {
            self.search_state = SearchState::Idle;
            return;
        }
        self.load_more_if_needed(LoadTrigger::Search { token });
    }

    fn continue_search_if_token_matches(&mut self, completed_token: Option<usize>) {
        let Some(active) = self.search_state.active_token() else {
            return;
        };
        if let Some(token) = completed_token
            && token != active
        {
            return;
        }
        self.continue_search_if_needed();
    }

    fn ensure_selected_visible(&mut self) {
        if self.filtered_rows.is_empty() {
            self.scroll_top = 0;
            return;
        }
        let capacity = self.view_rows.unwrap_or(self.filtered_rows.len()).max(1);

        if self.selected < self.scroll_top {
            self.scroll_top = self.selected;
        } else {
            let last_visible = self.scroll_top.saturating_add(capacity - 1);
            if self.selected > last_visible {
                self.scroll_top = self.selected.saturating_sub(capacity - 1);
            }
        }

        let max_start = self.filtered_rows.len().saturating_sub(capacity);
        if self.scroll_top > max_start {
            self.scroll_top = max_start;
        }
    }

    fn ensure_minimum_rows_for_view(&mut self, minimum_rows: usize) {
        if minimum_rows == 0 {
            return;
        }
        if self.filtered_rows.len() >= minimum_rows {
            return;
        }
        if self.pagination.loading.is_pending() || self.pagination.next_cursor.is_none() {
            return;
        }
        if let Some(token) = self.search_state.active_token() {
            self.load_more_if_needed(LoadTrigger::Search { token });
        } else {
            self.load_more_if_needed(LoadTrigger::Scroll);
        }
    }

    fn update_view_rows(&mut self, rows: usize) {
        self.view_rows = if rows == 0 { None } else { Some(rows) };
        self.ensure_selected_visible();
    }

    fn maybe_load_more_for_scroll(&mut self) {
        if self.pagination.loading.is_pending() {
            return;
        }
        if self.pagination.next_cursor.is_none() {
            return;
        }
        if self.filtered_rows.is_empty() {
            return;
        }
        let remaining = self.filtered_rows.len().saturating_sub(self.selected + 1);
        if remaining <= LOAD_NEAR_THRESHOLD {
            self.load_more_if_needed(LoadTrigger::Scroll);
        }
    }

    fn load_more_if_needed(&mut self, trigger: LoadTrigger) {
        if self.pagination.loading.is_pending() {
            return;
        }
        let Some(cursor) = self.pagination.next_cursor.clone() else {
            return;
        };
        let request_token = self.allocate_request_token();
        let search_token = match trigger {
            LoadTrigger::Scroll => None,
            LoadTrigger::Search { token } => Some(token),
        };
        self.pagination.loading = LoadingState::Pending(PendingLoad {
            request_token,
            search_token,
        });
        self.request_frame();

        (self.page_loader)(PageLoadRequest {
            cursor: Some(cursor),
            request_token,
            search_token,
            cwd_filter: self.current_cwd_filter(),
            purpose: LoadPurpose::Active,
            provider_filter: self.provider_filter.clone(),
            sort_key: self.sort_key,
        });
    }

    fn allocate_request_token(&mut self) -> usize {
        let token = self.next_request_token;
        self.next_request_token = self.next_request_token.wrapping_add(1);
        token
    }

    fn allocate_search_token(&mut self) -> usize {
        let token = self.next_search_token;
        self.next_search_token = self.next_search_token.wrapping_add(1);
        token
    }

    /// Cycles the sort order between creation time and last-updated time.
    ///
    /// Triggers a full reload because the backend must re-sort all sessions.
    /// The existing `all_rows` are cleared and pagination restarts from the
    /// beginning with the new sort key.
    fn toggle_sort_key(&mut self) {
        self.sort_key = match self.sort_key {
            ThreadSortKey::CreatedAt => ThreadSortKey::UpdatedAt,
            ThreadSortKey::UpdatedAt => ThreadSortKey::CreatedAt,
        };
        self.start_initial_load();
    }
}

fn row_from_app_server_thread(thread: Thread, open_state: SessionOpenState) -> Option<Row> {
    let thread_id = match ThreadId::from_string(&thread.id) {
        Ok(thread_id) => thread_id,
        Err(err) => {
            warn!(thread_id = thread.id, %err, "Skipping app-server picker row with invalid id");
            return None;
        }
    };
    let preview = thread.preview.trim();
    Some(Row {
        path: thread.path,
        preview: if preview.is_empty() {
            String::from("(no message yet)")
        } else {
            preview.to_string()
        },
        thread_id: Some(thread_id),
        thread_name: thread.name,
        user_message_count: thread.user_message_count,
        created_at: chrono::DateTime::from_timestamp(thread.created_at, 0)
            .map(|dt| dt.with_timezone(&Utc)),
        updated_at: chrono::DateTime::from_timestamp(thread.updated_at, 0)
            .map(|dt| dt.with_timezone(&Utc)),
        cwd: Some(thread.cwd.to_path_buf()),
        git_branch: thread.git_info.and_then(|git_info| git_info.branch),
        open_state,
    })
}

fn thread_list_params(
    cursor: Option<String>,
    cwd_filter: Option<&Path>,
    provider_filter: ProviderFilter,
    sort_key: ThreadSortKey,
    include_non_interactive: bool,
) -> ThreadListParams {
    ThreadListParams {
        cursor,
        limit: Some(PAGE_SIZE as u32),
        sort_key: Some(sort_key),
        sort_direction: None,
        model_providers: match provider_filter {
            ProviderFilter::Any => None,
            ProviderFilter::MatchDefault(default_provider) => Some(vec![default_provider]),
        },
        source_kinds: (!include_non_interactive)
            .then_some(vec![ThreadSourceKind::Cli, ThreadSourceKind::VsCode]),
        archived: Some(false),
        cwd: cwd_filter.map(|cwd| ThreadListCwdFilter::One(cwd.to_string_lossy().into_owned())),
        use_state_db_only: false,
        search_term: None,
    }
}

fn paths_match(a: &Path, b: &Path) -> bool {
    path_utils::paths_match_after_normalization(a, b)
}

#[cfg_attr(not(test), allow(dead_code))]
fn parse_timestamp_str(ts: &str) -> Option<DateTime<Utc>> {
    chrono::DateTime::parse_from_rfc3339(ts)
        .map(|dt| dt.with_timezone(&Utc))
        .ok()
}

fn draw_picker(tui: &mut Tui, state: &PickerState) -> std::io::Result<()> {
    // Render full-screen overlay
    let height = tui.terminal.size()?.height;
    tui.draw(height, |frame| {
        let area = frame.area();
        let [header, search, columns, list, hint] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(area.height.saturating_sub(4)),
            Constraint::Length(1),
        ])
        .areas(area);

        // Header
        let header_line: Line = vec![
            state.action.title().bold().cyan(),
            "  ".into(),
            "Sort:".dim(),
            " ".into(),
            sort_key_label(state.sort_key).magenta(),
            if state.custom_titles_only {
                "  Filter:".dim()
            } else {
                "".into()
            },
            if state.custom_titles_only {
                " custom titles".magenta()
            } else {
                "".into()
            },
            if state.show_all {
                "  Scope:".dim()
            } else {
                "".into()
            },
            if state.show_all {
                " all dirs".magenta()
            } else {
                "".into()
            },
        ]
        .into();
        frame.render_widget_ref(header_line, header);

        // Search line
        frame.render_widget_ref(search_line(state), search);

        let metrics = calculate_column_metrics(
            &state.filtered_rows,
            state.show_all,
            state.relative_time_reference.unwrap_or_else(Utc::now),
        );

        // Column headers and list
        render_column_headers(frame, columns, &metrics, state.sort_key);
        render_list(frame, list, state, &metrics);

        // Hint line
        let action_label = state.action.action_label();
        let hint_line: Line = vec![
            key_hint::plain(KeyCode::Enter).into(),
            format!(" to {action_label} ").dim(),
            "    ".dim(),
            key_hint::plain(KeyCode::Esc).into(),
            " to start new ".dim(),
            "    ".dim(),
            key_hint::ctrl(KeyCode::Char('c')).into(),
            " to quit ".dim(),
            "    ".dim(),
            key_hint::plain(KeyCode::Tab).into(),
            " to toggle sort ".dim(),
            "    ".dim(),
            key_hint::ctrl(KeyCode::Char('t')).into(),
            " titles only ".dim(),
            "    ".dim(),
            key_hint::ctrl(KeyCode::Char('a')).into(),
            " all dirs ".dim(),
            "    ".dim(),
            key_hint::plain(KeyCode::Up).into(),
            "/".dim(),
            key_hint::plain(KeyCode::Down).into(),
            " to browse".dim(),
        ]
        .into();
        frame.render_widget_ref(hint_line, hint);
    })
}

fn search_line(state: &PickerState) -> Line<'_> {
    if let Some(error) = state.inline_error.as_deref() {
        return Line::from(error.red());
    }
    if state.query.is_empty() {
        return Line::from("Type to search".dim());
    }
    Line::from(format!("Search: {}", state.query))
}

fn render_list(
    frame: &mut crate::custom_terminal::Frame,
    area: Rect,
    state: &PickerState,
    metrics: &ColumnMetrics,
) {
    if area.height == 0 {
        return;
    }

    let rows = &state.filtered_rows;
    if rows.is_empty() {
        let message = render_empty_state_line(state);
        frame.render_widget_ref(message, area);
        return;
    }

    let capacity = area.height as usize;
    let start = state.scroll_top.min(rows.len().saturating_sub(1));
    let end = rows.len().min(start + capacity);
    let labels = &metrics.labels;
    let mut y = area.y;

    let visibility = column_visibility(area.width, metrics, state.sort_key);
    let max_created_width = metrics.max_created_width;
    let max_updated_width = metrics.max_updated_width;
    let max_message_count_width = metrics.max_message_count_width;
    let max_open_state_width = metrics.max_open_state_width;
    let max_session_id_suffix_width = metrics.max_session_id_suffix_width;
    let max_branch_width = metrics.max_branch_width;
    let max_cwd_width = metrics.max_cwd_width;

    for (
        idx,
        (
            row,
            (
                created_label,
                updated_label,
                message_count_label,
                open_state_label,
                session_id_suffix_label,
                branch_label,
                cwd_label,
            ),
        ),
    ) in rows[start..end]
        .iter()
        .zip(labels[start..end].iter())
        .enumerate()
    {
        let is_sel = start + idx == state.selected;
        let marker = if is_sel { "> ".bold() } else { "  ".into() };
        let marker_width = 2usize;
        let created_span = if visibility.show_created {
            let span = Span::from(format!("{created_label:<max_created_width$}"));
            Some(if row.has_custom_title() {
                span
            } else {
                span.dim()
            })
        } else {
            None
        };
        let updated_span = if visibility.show_updated {
            let span = Span::from(format!("{updated_label:<max_updated_width$}"));
            Some(if row.has_custom_title() {
                span
            } else {
                span.dim()
            })
        } else {
            None
        };
        let branch_span = if !visibility.show_branch {
            None
        } else if branch_label.is_empty() {
            Some(
                Span::from(format!(
                    "{empty:<width$}",
                    empty = "-",
                    width = max_branch_width
                ))
                .dim(),
            )
        } else {
            Some(Span::from(format!("{branch_label:<max_branch_width$}")).cyan())
        };
        let cwd_span = if !visibility.show_cwd {
            None
        } else if cwd_label.is_empty() {
            Some(
                Span::from(format!(
                    "{empty:<width$}",
                    empty = "-",
                    width = max_cwd_width
                ))
                .dim(),
            )
        } else {
            let span = Span::from(format!("{cwd_label:<max_cwd_width$}"));
            Some(if row.has_custom_title() {
                span
            } else {
                span.dim()
            })
        };
        let open_state_span = match row.open_state {
            SessionOpenState::Open => {
                Span::from(format!("{open_state_label:<max_open_state_width$}")).green()
            }
            SessionOpenState::Closed => {
                Span::from(format!("{open_state_label:<max_open_state_width$}")).dim()
            }
        };

        let mut preview_width = area.width as usize;
        preview_width = preview_width.saturating_sub(marker_width);
        if visibility.show_created {
            preview_width = preview_width.saturating_sub(max_created_width + 2);
        }
        if visibility.show_updated {
            preview_width = preview_width.saturating_sub(max_updated_width + 2);
        }
        preview_width = preview_width.saturating_sub(max_message_count_width + 2);
        preview_width = preview_width.saturating_sub(max_open_state_width + 2);
        preview_width = preview_width.saturating_sub(max_session_id_suffix_width + 2);
        if visibility.show_branch {
            preview_width = preview_width.saturating_sub(max_branch_width + 2);
        }
        if visibility.show_cwd {
            preview_width = preview_width.saturating_sub(max_cwd_width + 2);
        }
        let add_leading_gap = !visibility.show_created
            && !visibility.show_updated
            && !visibility.show_branch
            && !visibility.show_cwd;
        if add_leading_gap {
            preview_width = preview_width.saturating_sub(2);
        }
        let (preview_prefix, preview_width) = if row.has_custom_title() && preview_width >= 2 {
            (Some("★ "), preview_width.saturating_sub(2))
        } else {
            (None, preview_width)
        };
        let preview = truncate_text(row.display_preview(), preview_width);
        let mut spans: Vec<Span> = vec![marker];
        if let Some(created) = created_span {
            spans.push(created);
            spans.push("  ".into());
        }
        if let Some(updated) = updated_span {
            spans.push(updated);
            spans.push("  ".into());
        }
        spans.push(Span::from(format!(
            "{message_count_label:>max_message_count_width$}"
        )));
        spans.push("  ".into());
        spans.push(open_state_span);
        spans.push("  ".into());
        spans.push(Span::from(format!(
            "{session_id_suffix_label:<max_session_id_suffix_width$}"
        )));
        spans.push("  ".into());
        if let Some(branch) = branch_span {
            spans.push(branch);
            spans.push("  ".into());
        }
        if let Some(cwd) = cwd_span {
            spans.push(cwd);
            spans.push("  ".into());
        }
        if add_leading_gap {
            spans.push("  ".into());
        }
        if let Some(prefix) = preview_prefix {
            spans.push(prefix.magenta());
        }
        spans.push(preview.into());
        if row.has_custom_title() {
            for span in &mut spans {
                span.style = span.style.add_modifier(Modifier::BOLD);
            }
        }
        let line: Line = spans.into();
        let rect = Rect::new(area.x, y, area.width, 1);
        frame.render_widget_ref(line, rect);
        y = y.saturating_add(1);
    }

    if state.pagination.loading.is_pending() && y < area.y.saturating_add(area.height) {
        let loading_line: Line = vec!["  ".into(), "Loading older sessions…".italic().dim()].into();
        let rect = Rect::new(area.x, y, area.width, 1);
        frame.render_widget_ref(loading_line, rect);
    }
}

fn render_empty_state_line(state: &PickerState) -> Line<'static> {
    if !state.query.is_empty() {
        if state.search_state.is_active()
            || (state.pagination.loading.is_pending() && state.pagination.next_cursor.is_some())
        {
            return vec!["Searching…".italic().dim()].into();
        }
        if state.pagination.reached_scan_cap {
            let msg = format!(
                "Search scanned first {} sessions; more may exist",
                state.pagination.num_scanned_files
            );
            return vec![Span::from(msg).italic().dim()].into();
        }
        return vec!["No results for your search".italic().dim()].into();
    }

    if state.pagination.loading.is_pending() {
        if state.all_rows.is_empty() && state.pagination.num_scanned_files == 0 {
            return vec!["Loading sessions…".italic().dim()].into();
        }
        return vec!["Loading older sessions…".italic().dim()].into();
    }

    if state.custom_titles_only {
        return vec!["No custom-titled sessions".italic().dim()].into();
    }

    vec!["No sessions yet".italic().dim()].into()
}

fn human_time_ago(ts: DateTime<Utc>, reference_now: DateTime<Utc>) -> String {
    let delta = reference_now - ts;
    let secs = delta.num_seconds();
    if secs < 60 {
        let n = secs.max(0);
        if n == 1 {
            format!("{n} second ago")
        } else {
            format!("{n} seconds ago")
        }
    } else if secs < 60 * 60 {
        let m = secs / 60;
        if m == 1 {
            format!("{m} minute ago")
        } else {
            format!("{m} minutes ago")
        }
    } else if secs < 60 * 60 * 24 {
        let h = secs / 3600;
        if h == 1 {
            format!("{h} hour ago")
        } else {
            format!("{h} hours ago")
        }
    } else {
        let d = secs / (60 * 60 * 24);
        if d == 1 {
            format!("{d} day ago")
        } else {
            format!("{d} days ago")
        }
    }
}

fn format_updated_label_at(row: &Row, reference_now: DateTime<Utc>) -> String {
    match (row.updated_at, row.created_at) {
        (Some(updated), _) => human_time_ago(updated, reference_now),
        (None, Some(created)) => human_time_ago(created, reference_now),
        (None, None) => "-".to_string(),
    }
}

fn format_created_label_at(row: &Row, reference_now: DateTime<Utc>) -> String {
    match row.created_at {
        Some(created) => human_time_ago(created, reference_now),
        None => "-".to_string(),
    }
}

fn render_column_headers(
    frame: &mut crate::custom_terminal::Frame,
    area: Rect,
    metrics: &ColumnMetrics,
    sort_key: ThreadSortKey,
) {
    if area.height == 0 {
        return;
    }

    let mut spans: Vec<Span> = vec!["  ".into()];
    let visibility = column_visibility(area.width, metrics, sort_key);
    if visibility.show_created {
        let label = format!(
            "{text:<width$}",
            text = CREATED_COLUMN_LABEL,
            width = metrics.max_created_width
        );
        spans.push(Span::from(label).bold());
        spans.push("  ".into());
    }
    if visibility.show_updated {
        let label = format!(
            "{text:<width$}",
            text = UPDATED_COLUMN_LABEL,
            width = metrics.max_updated_width
        );
        spans.push(Span::from(label).bold());
        spans.push("  ".into());
    }
    let label = format!(
        "{text:>width$}",
        text = "Msgs",
        width = metrics.max_message_count_width
    );
    spans.push(Span::from(label).bold());
    spans.push("  ".into());
    let label = format!(
        "{text:<width$}",
        text = "Open",
        width = metrics.max_open_state_width
    );
    spans.push(Span::from(label).bold());
    spans.push("  ".into());
    let label = format!(
        "{text:<width$}",
        text = "ID",
        width = metrics.max_session_id_suffix_width
    );
    spans.push(Span::from(label).bold());
    spans.push("  ".into());
    if visibility.show_branch {
        let label = format!(
            "{text:<width$}",
            text = "Branch",
            width = metrics.max_branch_width
        );
        spans.push(Span::from(label).bold());
        spans.push("  ".into());
    }
    if visibility.show_cwd {
        let label = format!(
            "{text:<width$}",
            text = "CWD",
            width = metrics.max_cwd_width
        );
        spans.push(Span::from(label).bold());
        spans.push("  ".into());
    }
    spans.push("Conversation".bold());
    frame.render_widget_ref(Line::from(spans), area);
}

/// Pre-computed column widths and formatted labels for all visible rows.
///
/// Widths are measured in Unicode display width (not byte length) so columns
/// align correctly when labels contain non-ASCII characters.
struct ColumnMetrics {
    max_created_width: usize,
    max_updated_width: usize,
    max_message_count_width: usize,
    max_open_state_width: usize,
    max_session_id_suffix_width: usize,
    max_branch_width: usize,
    max_cwd_width: usize,
    /// (created_label, updated_label, message_count_label, open_state_label,
    /// session_id_suffix_label, branch_label, cwd_label) per row.
    labels: Vec<(String, String, String, String, String, String, String)>,
}

/// Determines which columns to render given available terminal width.
///
/// When the terminal is narrow, only one timestamp column is shown (whichever
/// matches the current sort key). Branch and CWD are hidden if their max
/// widths are zero (no data to show).
#[derive(Debug, PartialEq, Eq)]
struct ColumnVisibility {
    show_created: bool,
    show_updated: bool,
    show_branch: bool,
    show_cwd: bool,
}

fn calculate_column_metrics(
    rows: &[Row],
    include_cwd: bool,
    reference_now: DateTime<Utc>,
) -> ColumnMetrics {
    fn right_elide(s: &str, max: usize) -> String {
        if s.chars().count() <= max {
            return s.to_string();
        }
        if max <= 1 {
            return "…".to_string();
        }
        let tail_len = max - 1;
        let tail: String = s
            .chars()
            .rev()
            .take(tail_len)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        format!("…{tail}")
    }

    let cwd_labels = cwd_labels_for_rows(rows, include_cwd);
    let mut labels: Vec<(String, String, String, String, String, String, String)> =
        Vec::with_capacity(rows.len());
    let mut max_created_width = UnicodeWidthStr::width(CREATED_COLUMN_LABEL);
    let mut max_updated_width = UnicodeWidthStr::width(UPDATED_COLUMN_LABEL);
    let mut max_message_count_width = UnicodeWidthStr::width("Msgs");
    let mut max_open_state_width = UnicodeWidthStr::width("Open");
    let mut max_session_id_suffix_width = UnicodeWidthStr::width("ID");
    let mut max_branch_width = UnicodeWidthStr::width("Branch");
    let mut max_cwd_width = if include_cwd {
        UnicodeWidthStr::width("CWD")
    } else {
        0
    };

    for (row, cwd_raw) in rows.iter().zip(cwd_labels) {
        let created = format_created_label_at(row, reference_now);
        let updated = format_updated_label_at(row, reference_now);
        let message_count = row.user_message_count.to_string();
        let open_state = row.open_state.label().to_string();
        let session_id_suffix = row.session_id_suffix();
        let branch_raw = row.git_branch.clone().unwrap_or_default();
        let branch = right_elide(&branch_raw, /*max*/ 24);
        let cwd = if include_cwd {
            right_elide(&cwd_raw, /*max*/ 24)
        } else {
            String::new()
        };
        max_created_width = max_created_width.max(UnicodeWidthStr::width(created.as_str()));
        max_updated_width = max_updated_width.max(UnicodeWidthStr::width(updated.as_str()));
        max_message_count_width =
            max_message_count_width.max(UnicodeWidthStr::width(message_count.as_str()));
        max_open_state_width =
            max_open_state_width.max(UnicodeWidthStr::width(open_state.as_str()));
        max_session_id_suffix_width =
            max_session_id_suffix_width.max(UnicodeWidthStr::width(session_id_suffix.as_str()));
        max_branch_width = max_branch_width.max(UnicodeWidthStr::width(branch.as_str()));
        max_cwd_width = max_cwd_width.max(UnicodeWidthStr::width(cwd.as_str()));
        labels.push((
            created,
            updated,
            message_count,
            open_state,
            session_id_suffix,
            branch,
            cwd,
        ));
    }

    ColumnMetrics {
        max_created_width,
        max_updated_width,
        max_message_count_width,
        max_open_state_width,
        max_session_id_suffix_width,
        max_branch_width,
        max_cwd_width,
        labels,
    }
}

fn cwd_labels_for_rows(rows: &[Row], include_cwd: bool) -> Vec<String> {
    if !include_cwd {
        return vec![String::new(); rows.len()];
    }

    let mut paths_by_basename: HashMap<String, HashSet<PathBuf>> = HashMap::new();
    for cwd in rows.iter().filter_map(|row| row.cwd.as_deref()) {
        let Some(basename) = cwd_basename(cwd) else {
            continue;
        };
        paths_by_basename
            .entry(basename)
            .or_default()
            .insert(cwd.to_path_buf());
    }

    rows.iter()
        .map(|row| {
            let Some(cwd) = row.cwd.as_deref() else {
                return String::new();
            };
            let Some(basename) = cwd_basename(cwd) else {
                return abbreviated_cwd_path(cwd);
            };
            if paths_by_basename
                .get(&basename)
                .is_some_and(|paths| paths.len() == 1)
            {
                basename
            } else {
                abbreviated_cwd_path(cwd)
            }
        })
        .collect()
}

fn cwd_basename(cwd: &Path) -> Option<String> {
    cwd.file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

fn abbreviated_cwd_path(cwd: &Path) -> String {
    relativize_to_home(cwd)
        .map(|relative| {
            if relative.as_os_str().is_empty() {
                "~".to_string()
            } else {
                PathBuf::from_iter([Path::new("~"), relative.as_path()])
                    .display()
                    .to_string()
            }
        })
        .unwrap_or_else(|| cwd.display().to_string())
}

/// Computes which columns fit in the available width.
///
/// The algorithm reserves at least `MIN_PREVIEW_WIDTH` characters for the
/// conversation preview. If both timestamp columns don't fit, only the one
/// matching the current sort key is shown.
fn column_visibility(
    area_width: u16,
    metrics: &ColumnMetrics,
    sort_key: ThreadSortKey,
) -> ColumnVisibility {
    const MIN_PREVIEW_WIDTH: usize = 10;

    let show_branch = metrics.max_branch_width > 0;
    let show_cwd = metrics.max_cwd_width > 0;

    // Calculate remaining width after all optional columns.
    let mut preview_width = area_width as usize;
    preview_width = preview_width.saturating_sub(2); // marker
    if metrics.max_created_width > 0 {
        preview_width = preview_width.saturating_sub(metrics.max_created_width + 2);
    }
    if metrics.max_updated_width > 0 {
        preview_width = preview_width.saturating_sub(metrics.max_updated_width + 2);
    }
    preview_width = preview_width.saturating_sub(metrics.max_message_count_width + 2);
    preview_width = preview_width.saturating_sub(metrics.max_open_state_width + 2);
    preview_width = preview_width.saturating_sub(metrics.max_session_id_suffix_width + 2);
    if show_branch {
        preview_width = preview_width.saturating_sub(metrics.max_branch_width + 2);
    }
    if show_cwd {
        preview_width = preview_width.saturating_sub(metrics.max_cwd_width + 2);
    }

    // If preview would be too narrow, hide the non-active timestamp column.
    let show_both = preview_width >= MIN_PREVIEW_WIDTH;
    let show_created = if show_both {
        metrics.max_created_width > 0
    } else {
        sort_key == ThreadSortKey::CreatedAt
    };
    let show_updated = if show_both {
        metrics.max_updated_width > 0
    } else {
        sort_key == ThreadSortKey::UpdatedAt
    };

    ColumnVisibility {
        show_created,
        show_updated,
        show_branch,
        show_cwd,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use codex_protocol::ThreadId;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;

    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::Mutex;

    fn page(
        rows: Vec<Row>,
        next_cursor: Option<&str>,
        num_scanned_files: usize,
        reached_scan_cap: bool,
    ) -> PickerPage {
        PickerPage {
            rows,
            next_cursor: next_cursor.map(|cursor| PageCursor::AppServer(cursor.to_string())),
            num_scanned_files,
            reached_scan_cap,
        }
    }

    fn make_row(path: &str, ts: &str, preview: &str) -> Row {
        let timestamp = parse_timestamp_str(ts).expect("timestamp should parse");
        Row {
            path: Some(PathBuf::from(path)),
            preview: preview.to_string(),
            thread_id: None,
            thread_name: None,
            user_message_count: 0,
            created_at: Some(timestamp),
            updated_at: Some(timestamp),
            cwd: None,
            git_branch: None,
            open_state: SessionOpenState::Closed,
        }
    }

    fn app_server_thread(thread_id: ThreadId, status: ThreadStatus) -> Thread {
        Thread {
            id: thread_id.to_string(),
            forked_from_id: None,
            preview: String::from("remote thread"),
            ephemeral: false,
            model_provider: String::from("openai"),
            created_at: 1,
            updated_at: 2,
            status,
            path: None,
            cwd: test_path_buf("/tmp").abs(),
            cli_version: String::from("0.0.0"),
            source: codex_app_server_protocol::SessionSource::Cli,
            agent_nickname: None,
            agent_role: None,
            git_info: None,
            name: Some(String::from("Named thread")),
            user_message_count: 0,
            turns: Vec::new(),
        }
    }

    #[tokio::test]
    async fn control_q_and_control_x_exit_picker_like_control_c() {
        for code in [KeyCode::Char('c'), KeyCode::Char('q'), KeyCode::Char('x')] {
            let loader: PageLoader = Arc::new(|_| {});
            let mut state = PickerState::new(
                FrameRequester::test_dummy(),
                loader,
                ProviderFilter::MatchDefault(String::from("openai")),
                /*show_all*/ true,
                /*filter_cwd*/ None,
                SessionPickerAction::Resume,
            );

            let selection = state
                .handle_key(KeyEvent::new(code, KeyModifiers::CONTROL))
                .await
                .expect("exit shortcut should not abort the picker");

            assert!(matches!(selection, Some(SessionSelection::Exit)));
        }
    }

    #[test]
    fn row_display_preview_prefers_thread_name() {
        let row = Row {
            path: Some(PathBuf::from("/tmp/a.jsonl")),
            preview: String::from("first message"),
            thread_id: None,
            thread_name: Some(String::from("My session")),
            user_message_count: 0,
            created_at: None,
            updated_at: None,
            cwd: None,
            git_branch: None,
            open_state: SessionOpenState::Closed,
        };

        assert_eq!(row.display_preview(), "My session");
    }

    #[test]
    fn row_display_preview_preserves_leading_emoji_cluster() {
        let row = Row {
            path: Some(PathBuf::from("/tmp/a.jsonl")),
            preview: String::from("first message"),
            thread_id: None,
            thread_name: Some(String::from("🧪 🧭 🔎 My session")),
            user_message_count: 0,
            created_at: None,
            updated_at: None,
            cwd: None,
            git_branch: None,
            open_state: SessionOpenState::Closed,
        };

        assert_eq!(row.display_preview(), "🧪 🧭 🔎 My session");
    }

    #[test]
    fn row_with_thread_name_is_custom_titled() {
        let row = Row {
            path: Some(PathBuf::from("/tmp/a.jsonl")),
            preview: String::from("first message"),
            thread_id: None,
            thread_name: Some(String::from("My session")),
            user_message_count: 0,
            created_at: None,
            updated_at: None,
            cwd: None,
            git_branch: None,
            open_state: SessionOpenState::Closed,
        };

        assert!(row.has_custom_title());
    }

    #[test]
    fn row_session_id_suffix_uses_last_eight_characters() {
        let row = Row {
            path: Some(PathBuf::from("/tmp/a.jsonl")),
            preview: String::from("first message"),
            thread_id: Some(
                ThreadId::from_string("019dfe7f-c670-71b3-a1c8-ab8fac1bbd40")
                    .expect("valid thread id"),
            ),
            thread_name: None,
            user_message_count: 0,
            created_at: None,
            updated_at: None,
            cwd: None,
            git_branch: None,
            open_state: SessionOpenState::Closed,
        };

        assert_eq!(row.session_id_suffix(), "…ac1bbd40");
    }

    #[test]
    fn row_query_matches_session_id_fragments() {
        let row = Row {
            path: Some(PathBuf::from("/tmp/a.jsonl")),
            preview: String::from("first message"),
            thread_id: Some(
                ThreadId::from_string("019dfe7f-c670-71b3-a1c8-ab8fac1bbd40")
                    .expect("valid thread id"),
            ),
            thread_name: None,
            user_message_count: 0,
            created_at: None,
            updated_at: None,
            cwd: None,
            git_branch: None,
            open_state: SessionOpenState::Closed,
        };

        assert!(row.matches_query("ac1"));
        assert!(row.matches_query("a1c8ab8f"));
        assert!(!row.matches_query("deadbeef"));
    }

    #[test]
    fn local_picker_thread_list_params_include_cwd_filter() {
        let cwd_filter = picker_cwd_filter(
            Path::new("/tmp/project"),
            /*show_all*/ false,
            /*is_remote*/ false,
            /*remote_cwd_override*/ None,
        );
        let params = thread_list_params(
            Some(String::from("cursor-1")),
            cwd_filter.as_deref(),
            ProviderFilter::MatchDefault(String::from("openai")),
            ThreadSortKey::UpdatedAt,
            /*include_non_interactive*/ false,
        );

        assert_eq!(
            params.cwd,
            Some(ThreadListCwdFilter::One(String::from("/tmp/project")))
        );
    }

    #[test]
    fn remote_thread_list_params_omit_provider_filter() {
        let params = thread_list_params(
            Some(String::from("cursor-1")),
            Some(Path::new("repo/on/server")),
            ProviderFilter::Any,
            ThreadSortKey::UpdatedAt,
            /*include_non_interactive*/ false,
        );

        assert_eq!(params.cursor, Some(String::from("cursor-1")));
        assert_eq!(params.model_providers, None);
        assert_eq!(
            params.source_kinds,
            Some(vec![ThreadSourceKind::Cli, ThreadSourceKind::VsCode])
        );
        assert_eq!(
            params.cwd,
            Some(ThreadListCwdFilter::One(String::from("repo/on/server")))
        );
    }

    #[test]
    fn remote_thread_list_params_can_include_non_interactive_sources() {
        let params = thread_list_params(
            Some(String::from("cursor-1")),
            /*cwd_filter*/ None,
            ProviderFilter::Any,
            ThreadSortKey::UpdatedAt,
            /*include_non_interactive*/ true,
        );

        assert_eq!(params.cursor, Some(String::from("cursor-1")));
        assert_eq!(params.model_providers, None);
        assert_eq!(params.source_kinds, None);
    }

    #[test]
    fn remote_picker_does_not_filter_rows_by_local_cwd() {
        let loader: PageLoader = Arc::new(|_| {});
        let state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::Any,
            /*show_all*/ false,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );
        let row = Row {
            path: None,
            preview: String::from("remote session"),
            thread_id: Some(ThreadId::new()),
            thread_name: None,
            user_message_count: 0,
            created_at: None,
            updated_at: None,
            cwd: Some(PathBuf::from("/srv/remote-project")),
            git_branch: None,
            open_state: SessionOpenState::Closed,
        };

        assert!(state.row_matches_filter(&row));
    }

    #[test]
    fn cwd_labels_use_unique_basenames_when_possible() {
        let rows = vec![
            Row {
                path: None,
                preview: String::new(),
                thread_id: None,
                thread_name: None,
                user_message_count: 0,
                created_at: None,
                updated_at: None,
                cwd: Some(PathBuf::from("/Users/pc/code/codex")),
                git_branch: None,
                open_state: SessionOpenState::Closed,
            },
            Row {
                path: None,
                preview: String::new(),
                thread_id: None,
                thread_name: None,
                user_message_count: 0,
                created_at: None,
                updated_at: None,
                cwd: Some(PathBuf::from("/tmp/other-project")),
                git_branch: None,
                open_state: SessionOpenState::Closed,
            },
        ];

        assert_eq!(
            cwd_labels_for_rows(&rows, /*include_cwd*/ true),
            vec![String::from("codex"), String::from("other-project")]
        );
    }

    #[test]
    fn cwd_labels_keep_repeated_same_directory_short() {
        let rows = vec![
            Row {
                path: None,
                preview: String::new(),
                thread_id: None,
                thread_name: None,
                user_message_count: 0,
                created_at: None,
                updated_at: None,
                cwd: Some(PathBuf::from("/Users/pc/code/codex")),
                git_branch: None,
                open_state: SessionOpenState::Closed,
            },
            Row {
                path: None,
                preview: String::new(),
                thread_id: None,
                thread_name: None,
                user_message_count: 0,
                created_at: None,
                updated_at: None,
                cwd: Some(PathBuf::from("/Users/pc/code/codex")),
                git_branch: None,
                open_state: SessionOpenState::Closed,
            },
        ];

        assert_eq!(
            cwd_labels_for_rows(&rows, /*include_cwd*/ true),
            vec![String::from("codex"), String::from("codex")]
        );
    }

    #[test]
    fn cwd_labels_expand_duplicate_basenames_and_tilde_abbreviate_home() {
        let home = dirs::home_dir().expect("home directory should be available");
        let rows = vec![
            Row {
                path: None,
                preview: String::new(),
                thread_id: None,
                thread_name: None,
                user_message_count: 0,
                created_at: None,
                updated_at: None,
                cwd: Some(home.join("code/codex")),
                git_branch: None,
                open_state: SessionOpenState::Closed,
            },
            Row {
                path: None,
                preview: String::new(),
                thread_id: None,
                thread_name: None,
                user_message_count: 0,
                created_at: None,
                updated_at: None,
                cwd: Some(PathBuf::from("/tmp/codex")),
                git_branch: None,
                open_state: SessionOpenState::Closed,
            },
        ];

        assert_eq!(
            cwd_labels_for_rows(&rows, /*include_cwd*/ true),
            vec![String::from("~/code/codex"), String::from("/tmp/codex")]
        );
    }

    #[test]
    fn resume_table_snapshot() {
        use crate::custom_terminal::Terminal;
        use crate::test_backend::VT100Backend;
        use ratatui::layout::Constraint;
        use ratatui::layout::Layout;

        let loader: PageLoader = Arc::new(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );

        let now = Utc::now();
        let rows = vec![
            Row {
                path: Some(PathBuf::from("/tmp/a.jsonl")),
                preview: String::from("Fix resume picker timestamps"),
                thread_id: Some(
                    ThreadId::from_string("019df98c-eb13-7963-8f32-1f1ae9c6cc6c")
                        .expect("valid thread id"),
                ),
                thread_name: None,
                user_message_count: 0,
                created_at: Some(now - Duration::minutes(16)),
                updated_at: Some(now - Duration::seconds(42)),
                cwd: None,
                git_branch: None,
                open_state: SessionOpenState::Closed,
            },
            Row {
                path: Some(PathBuf::from("/tmp/b.jsonl")),
                preview: String::from("Investigate lazy pagination cap"),
                thread_id: Some(
                    ThreadId::from_string("019dfe7f-c670-71b3-a1c8-ab8fac1bbd40")
                        .expect("valid thread id"),
                ),
                thread_name: Some(String::from("Resume picker cleanup")),
                user_message_count: 12,
                created_at: Some(now - Duration::hours(1)),
                updated_at: Some(now - Duration::minutes(35)),
                cwd: None,
                git_branch: None,
                open_state: SessionOpenState::Open,
            },
            Row {
                path: Some(PathBuf::from("/tmp/c.jsonl")),
                preview: String::from("Explain the codebase"),
                thread_id: None,
                thread_name: None,
                user_message_count: 0,
                created_at: Some(now - Duration::hours(2)),
                updated_at: Some(now - Duration::hours(2)),
                cwd: None,
                git_branch: None,
                open_state: SessionOpenState::Closed,
            },
        ];
        state.all_rows = rows.clone();
        state.filtered_rows = rows;
        state.view_rows = Some(3);
        state.selected = 1;
        state.scroll_top = 0;
        state.update_view_rows(/*rows*/ 3);

        state.relative_time_reference = Some(now);
        let metrics = calculate_column_metrics(&state.filtered_rows, state.show_all, now);

        let width: u16 = 80;
        let height: u16 = 6;
        let backend = VT100Backend::new(width, height);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, width, height));

        {
            let mut frame = terminal.get_frame();
            let area = frame.area();
            let segments =
                Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).split(area);
            render_column_headers(&mut frame, segments[0], &metrics, state.sort_key);
            render_list(&mut frame, segments[1], &state, &metrics);
        }
        terminal.flush().expect("flush");

        let snapshot = terminal.backend().to_string();
        assert_snapshot!("resume_picker_table", snapshot);
    }

    #[test]
    fn resume_search_error_snapshot() {
        use crate::custom_terminal::Terminal;
        use crate::test_backend::VT100Backend;

        let loader: PageLoader = Arc::new(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );
        state.inline_error = Some(String::from(
            "Failed to read session metadata from /tmp/missing.jsonl",
        ));

        let width: u16 = 80;
        let height: u16 = 1;
        let backend = VT100Backend::new(width, height);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, width, height));

        {
            let mut frame = terminal.get_frame();
            let line = search_line(&state);
            frame.render_widget_ref(line, frame.area());
        }
        terminal.flush().expect("flush");

        let snapshot = terminal.backend().to_string();
        assert_snapshot!("resume_picker_search_error", snapshot);
    }

    #[test]
    fn pageless_scrolling_deduplicates_and_keeps_order() {
        let loader: PageLoader = Arc::new(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );

        state.reset_pagination();
        state.ingest_page(page(
            vec![
                make_row("/tmp/a.jsonl", "2025-01-03T00:00:00Z", "third"),
                make_row("/tmp/b.jsonl", "2025-01-02T00:00:00Z", "second"),
            ],
            Some("2025-01-02T00:00:00Z"),
            /*num_scanned_files*/ 2,
            /*reached_scan_cap*/ false,
        ));

        state.ingest_page(page(
            vec![
                make_row("/tmp/a.jsonl", "2025-01-03T00:00:00Z", "duplicate"),
                make_row("/tmp/c.jsonl", "2025-01-01T00:00:00Z", "first"),
            ],
            Some("2025-01-01T00:00:00Z"),
            /*num_scanned_files*/ 2,
            /*reached_scan_cap*/ false,
        ));

        state.ingest_page(page(
            vec![make_row("/tmp/d.jsonl", "2024-12-31T23:00:00Z", "very old")],
            /*next_cursor*/ None,
            /*num_scanned_files*/ 1,
            /*reached_scan_cap*/ false,
        ));

        let previews: Vec<_> = state
            .filtered_rows
            .iter()
            .map(|row| row.preview.as_str())
            .collect();
        assert_eq!(previews, vec!["third", "second", "first", "very old"]);

        let unique_paths = state
            .filtered_rows
            .iter()
            .map(|row| row.path.clone())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(unique_paths.len(), 4);
    }

    #[test]
    fn ensure_minimum_rows_prefetches_when_underfilled() {
        let recorded_requests: Arc<Mutex<Vec<PageLoadRequest>>> = Arc::new(Mutex::new(Vec::new()));
        let request_sink = recorded_requests.clone();
        let loader: PageLoader = Arc::new(move |req: PageLoadRequest| {
            request_sink.lock().unwrap().push(req);
        });

        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );
        state.reset_pagination();
        state.ingest_page(page(
            vec![
                make_row("/tmp/a.jsonl", "2025-01-01T00:00:00Z", "one"),
                make_row("/tmp/b.jsonl", "2025-01-02T00:00:00Z", "two"),
            ],
            Some("2025-01-03T00:00:00Z"),
            /*num_scanned_files*/ 2,
            /*reached_scan_cap*/ false,
        ));

        assert!(recorded_requests.lock().unwrap().is_empty());
        state.ensure_minimum_rows_for_view(/*minimum_rows*/ 10);
        let guard = recorded_requests.lock().unwrap();
        assert_eq!(guard.len(), 1);
        assert!(guard[0].search_token.is_none());
    }

    #[test]
    fn column_visibility_hides_extra_date_column_when_narrow() {
        let metrics = ColumnMetrics {
            max_created_width: 8,
            max_updated_width: 12,
            max_message_count_width: 4,
            max_open_state_width: 4,
            max_session_id_suffix_width: 9,
            max_branch_width: 0,
            max_cwd_width: 0,
            labels: Vec::new(),
        };

        let created = column_visibility(/*area_width*/ 30, &metrics, ThreadSortKey::CreatedAt);
        assert_eq!(
            created,
            ColumnVisibility {
                show_created: true,
                show_updated: false,
                show_branch: false,
                show_cwd: false,
            }
        );

        let updated = column_visibility(/*area_width*/ 30, &metrics, ThreadSortKey::UpdatedAt);
        assert_eq!(
            updated,
            ColumnVisibility {
                show_created: false,
                show_updated: true,
                show_branch: false,
                show_cwd: false,
            }
        );

        let wide = column_visibility(/*area_width*/ 59, &metrics, ThreadSortKey::CreatedAt);
        assert_eq!(
            wide,
            ColumnVisibility {
                show_created: true,
                show_updated: true,
                show_branch: false,
                show_cwd: false,
            }
        );
    }

    #[tokio::test]
    async fn toggle_sort_key_reloads_with_new_sort() {
        let recorded_requests: Arc<Mutex<Vec<PageLoadRequest>>> = Arc::new(Mutex::new(Vec::new()));
        let request_sink = recorded_requests.clone();
        let loader: PageLoader = Arc::new(move |req: PageLoadRequest| {
            request_sink.lock().unwrap().push(req);
        });

        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );

        state.start_initial_load();
        {
            let guard = recorded_requests.lock().unwrap();
            assert_eq!(guard.len(), 1);
            assert_eq!(guard[0].sort_key, ThreadSortKey::UpdatedAt);
        }

        state
            .handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE))
            .await
            .unwrap();

        let guard = recorded_requests.lock().unwrap();
        assert_eq!(guard.len(), 2);
        assert_eq!(guard[1].sort_key, ThreadSortKey::CreatedAt);
    }

    #[tokio::test]
    async fn page_navigation_uses_view_rows() {
        let loader: PageLoader = Arc::new(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );

        let mut items = Vec::new();
        for idx in 0..20 {
            let ts = format!("2025-01-{:02}T00:00:00Z", idx + 1);
            let preview = format!("item-{idx}");
            let path = format!("/tmp/item-{idx}.jsonl");
            items.push(make_row(&path, &ts, &preview));
        }

        state.reset_pagination();
        state.ingest_page(page(
            items, /*next_cursor*/ None, /*num_scanned_files*/ 20,
            /*reached_scan_cap*/ false,
        ));
        state.update_view_rows(/*rows*/ 5);

        assert_eq!(state.selected, 0);
        state
            .handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE))
            .await
            .unwrap();
        assert_eq!(state.selected, 5);

        state
            .handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE))
            .await
            .unwrap();
        assert_eq!(state.selected, 10);

        state
            .handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE))
            .await
            .unwrap();
        assert_eq!(state.selected, 5);
    }

    #[tokio::test]
    async fn enter_on_row_without_resolvable_thread_id_shows_inline_error() {
        let loader: PageLoader = Arc::new(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );

        let row = Row {
            path: Some(PathBuf::from("/tmp/missing.jsonl")),
            preview: String::from("missing metadata"),
            thread_id: None,
            thread_name: None,
            user_message_count: 0,
            created_at: None,
            updated_at: None,
            cwd: None,
            git_branch: None,
            open_state: SessionOpenState::Closed,
        };
        state.all_rows = vec![row.clone()];
        state.filtered_rows = vec![row];

        let selection = state
            .handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .await
            .expect("enter should not abort the picker");

        assert!(selection.is_none());
        assert_eq!(
            state.inline_error,
            Some(String::from(
                "Failed to read session metadata from /tmp/missing.jsonl"
            ))
        );
    }

    #[tokio::test]
    async fn enter_on_pathless_thread_uses_thread_id() {
        let loader: PageLoader = Arc::new(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );
        let thread_id = ThreadId::new();
        let row = Row {
            path: None,
            preview: String::from("pathless thread"),
            thread_id: Some(thread_id),
            thread_name: None,
            user_message_count: 0,
            created_at: None,
            updated_at: None,
            cwd: None,
            git_branch: None,
            open_state: SessionOpenState::Closed,
        };
        state.all_rows = vec![row.clone()];
        state.filtered_rows = vec![row];

        let selection = state
            .handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .await
            .expect("enter should not abort the picker");

        match selection {
            Some(SessionSelection::Resume(SessionTarget {
                path: None,
                thread_id: selected_thread_id,
            })) => assert_eq!(selected_thread_id, thread_id),
            other => panic!("unexpected selection: {other:?}"),
        }
    }

    #[test]
    fn app_server_row_keeps_pathless_threads() {
        let thread_id = ThreadId::new();
        let thread = app_server_thread(thread_id, ThreadStatus::Idle);

        let row = row_from_app_server_thread(thread, SessionOpenState::Open)
            .expect("row should be preserved");

        assert_eq!(row.path, None);
        assert_eq!(row.thread_id, Some(thread_id));
        assert_eq!(row.thread_name, Some(String::from("Named thread")));
        assert_eq!(row.open_state, SessionOpenState::Open);
    }

    #[tokio::test]
    async fn remote_rows_mark_loaded_threads_open() {
        let open_thread_id = ThreadId::new();
        let closed_thread_id = ThreadId::new();
        let rows = rows_from_app_server_threads(
            vec![
                app_server_thread(open_thread_id, ThreadStatus::Idle),
                app_server_thread(closed_thread_id, ThreadStatus::NotLoaded),
            ],
            SessionPickerRuntime::Remote,
        )
        .await;

        assert_eq!(
            rows.into_iter()
                .map(|row| (row.thread_id, row.open_state))
                .collect::<Vec<_>>(),
            vec![
                (Some(open_thread_id), SessionOpenState::Open),
                (Some(closed_thread_id), SessionOpenState::Closed),
            ]
        );
    }

    #[tokio::test]
    async fn up_at_bottom_does_not_scroll_when_visible() {
        let loader: PageLoader = Arc::new(|_| {});
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );

        let mut items = Vec::new();
        for idx in 0..10 {
            let ts = format!("2025-02-{:02}T00:00:00Z", idx + 1);
            let preview = format!("item-{idx}");
            let path = format!("/tmp/item-{idx}.jsonl");
            items.push(make_row(&path, &ts, &preview));
        }

        state.reset_pagination();
        state.ingest_page(page(
            items, /*next_cursor*/ None, /*num_scanned_files*/ 10,
            /*reached_scan_cap*/ false,
        ));
        state.update_view_rows(/*rows*/ 5);

        state.selected = state.filtered_rows.len().saturating_sub(1);
        state.ensure_selected_visible();

        let initial_top = state.scroll_top;
        assert_eq!(initial_top, state.filtered_rows.len().saturating_sub(5));

        state
            .handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE))
            .await
            .unwrap();

        assert_eq!(state.scroll_top, initial_top);
        assert_eq!(state.selected, state.filtered_rows.len().saturating_sub(2));
    }

    #[tokio::test]
    async fn set_query_loads_until_match_and_respects_scan_cap() {
        let recorded_requests: Arc<Mutex<Vec<PageLoadRequest>>> = Arc::new(Mutex::new(Vec::new()));
        let request_sink = recorded_requests.clone();
        let loader: PageLoader = Arc::new(move |req: PageLoadRequest| {
            request_sink.lock().unwrap().push(req);
        });

        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*filter_cwd*/ None,
            SessionPickerAction::Resume,
        );
        state.reset_pagination();
        state.ingest_page(page(
            vec![make_row(
                "/tmp/start.jsonl",
                "2025-01-01T00:00:00Z",
                "alpha",
            )],
            Some("2025-01-02T00:00:00Z"),
            /*num_scanned_files*/ 1,
            /*reached_scan_cap*/ false,
        ));
        recorded_requests.lock().unwrap().clear();

        state.set_query("target".to_string());
        let first_request = {
            let guard = recorded_requests.lock().unwrap();
            assert_eq!(guard.len(), 1);
            guard[0].clone()
        };

        state
            .handle_background_event(BackgroundEvent::PageLoaded {
                purpose: LoadPurpose::Active,
                request_token: first_request.request_token,
                search_token: first_request.search_token,
                page: Ok(page(
                    vec![make_row("/tmp/beta.jsonl", "2025-01-02T00:00:00Z", "beta")],
                    Some("2025-01-03T00:00:00Z"),
                    /*num_scanned_files*/ 5,
                    /*reached_scan_cap*/ false,
                )),
            })
            .await
            .unwrap();

        let second_request = {
            let guard = recorded_requests.lock().unwrap();
            assert_eq!(guard.len(), 2);
            guard[1].clone()
        };
        assert!(state.search_state.is_active());
        assert!(state.filtered_rows.is_empty());

        state
            .handle_background_event(BackgroundEvent::PageLoaded {
                purpose: LoadPurpose::Active,
                request_token: second_request.request_token,
                search_token: second_request.search_token,
                page: Ok(page(
                    vec![make_row(
                        "/tmp/match.jsonl",
                        "2025-01-03T00:00:00Z",
                        "target log",
                    )],
                    Some("2025-01-04T00:00:00Z"),
                    /*num_scanned_files*/ 7,
                    /*reached_scan_cap*/ false,
                )),
            })
            .await
            .unwrap();

        assert!(!state.filtered_rows.is_empty());
        assert!(!state.search_state.is_active());

        recorded_requests.lock().unwrap().clear();
        state.set_query("missing".to_string());
        let active_request = {
            let guard = recorded_requests.lock().unwrap();
            assert_eq!(guard.len(), 1);
            guard[0].clone()
        };

        state
            .handle_background_event(BackgroundEvent::PageLoaded {
                purpose: LoadPurpose::Active,
                request_token: second_request.request_token,
                search_token: second_request.search_token,
                page: Ok(page(
                    Vec::new(),
                    /*next_cursor*/ None,
                    /*num_scanned_files*/ 0,
                    /*reached_scan_cap*/ false,
                )),
            })
            .await
            .unwrap();
        assert_eq!(recorded_requests.lock().unwrap().len(), 1);

        state
            .handle_background_event(BackgroundEvent::PageLoaded {
                purpose: LoadPurpose::Active,
                request_token: active_request.request_token,
                search_token: active_request.search_token,
                page: Ok(page(
                    Vec::new(),
                    /*next_cursor*/ None,
                    /*num_scanned_files*/ 3,
                    /*reached_scan_cap*/ true,
                )),
            })
            .await
            .unwrap();

        assert!(state.filtered_rows.is_empty());
        assert!(!state.search_state.is_active());
        assert!(state.pagination.reached_scan_cap);
    }

    #[tokio::test]
    async fn toggle_custom_titles_only_loads_until_named_thread_is_found() {
        let recorded_requests: Arc<Mutex<Vec<PageLoadRequest>>> = Arc::new(Mutex::new(Vec::new()));
        let request_sink = recorded_requests.clone();
        let loader: PageLoader = Arc::new(move |req: PageLoadRequest| {
            request_sink.lock().unwrap().push(req);
        });
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ true,
            /*scope_cwd_filter*/ None,
            SessionPickerAction::Resume,
        );
        state.reset_pagination();
        state.ingest_page(page(
            vec![make_row(
                "/tmp/start.jsonl",
                "2025-01-01T00:00:00Z",
                "alpha",
            )],
            Some("2025-01-02T00:00:00Z"),
            /*num_scanned_files*/ 1,
            /*reached_scan_cap*/ false,
        ));
        recorded_requests.lock().unwrap().clear();

        state
            .handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL))
            .await
            .unwrap();

        let request = {
            let guard = recorded_requests.lock().unwrap();
            assert_eq!(guard.len(), 1);
            guard[0].clone()
        };
        assert!(state.custom_titles_only);
        assert!(state.search_state.is_active());
        assert!(state.filtered_rows.is_empty());

        let mut named_row = make_row("/tmp/named.jsonl", "2025-01-02T00:00:00Z", "beta");
        named_row.thread_name = Some(String::from("Named session"));
        state
            .handle_background_event(BackgroundEvent::PageLoaded {
                purpose: LoadPurpose::Active,
                request_token: request.request_token,
                search_token: request.search_token,
                page: Ok(page(
                    vec![named_row],
                    Some("2025-01-03T00:00:00Z"),
                    /*num_scanned_files*/ 1,
                    /*reached_scan_cap*/ false,
                )),
            })
            .await
            .unwrap();

        assert_eq!(state.filtered_rows.len(), 1);
        assert_eq!(state.filtered_rows[0].display_preview(), "Named session");
        assert!(!state.search_state.is_active());
    }

    #[tokio::test]
    async fn toggle_all_directories_reloads_without_cwd_filter() {
        let recorded_requests: Arc<Mutex<Vec<PageLoadRequest>>> = Arc::new(Mutex::new(Vec::new()));
        let request_sink = recorded_requests.clone();
        let loader: PageLoader = Arc::new(move |req: PageLoadRequest| {
            request_sink.lock().unwrap().push(req);
        });
        let mut state = PickerState::new(
            FrameRequester::test_dummy(),
            loader,
            ProviderFilter::MatchDefault(String::from("openai")),
            /*show_all*/ false,
            Some(PathBuf::from("/repo/current")),
            SessionPickerAction::Resume,
        );

        state.start_initial_load();
        assert_eq!(
            recorded_requests.lock().unwrap()[0].cwd_filter,
            Some(PathBuf::from("/repo/current"))
        );
        recorded_requests.lock().unwrap().clear();

        state
            .handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL))
            .await
            .unwrap();

        assert!(state.show_all);
        assert_eq!(recorded_requests.lock().unwrap()[0].cwd_filter, None);
    }
}
