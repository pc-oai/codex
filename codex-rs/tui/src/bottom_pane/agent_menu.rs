//! Floating agent chooser anchored near the prompt's left edge.
//!
//! This menu intentionally overlays the existing composer instead of replacing bottom-pane layout.
//! It provides a compact, keyboard-driven "context menu" shape for direct agent selection while
//! keeping the prompt box visible underneath.

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::scroll_state::ScrollState;
use crate::bottom_pane::selection_popup_common::GenericDisplayRow;
use crate::bottom_pane::selection_popup_common::render_rows_single_line;
use crate::multi_agents::agent_picker_status_dot_spans;
use crate::style::user_message_style;
use codex_protocol::ThreadId;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Clear;
use ratatui::widgets::Widget;
use std::path::PathBuf;

const AGENT_MENU_MIN_WIDTH: u16 = 22;
const AGENT_MENU_BORDER_HEIGHT: u16 = 2;
const AGENT_MENU_FOOTER_GAP: u16 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AgentMenuItem {
    pub(crate) thread_id: ThreadId,
    pub(crate) label: String,
    pub(crate) is_closed: bool,
}

pub(crate) struct AgentMenu {
    items: Vec<MenuItem>,
    state: ScrollState,
    app_event_tx: AppEventSender,
    empty_label: &'static str,
    snippet_response_offset: Option<usize>,
}

struct MenuItem {
    label: String,
    kind: MenuItemKind,
}

enum MenuItemKind {
    Agent {
        thread_id: ThreadId,
        is_closed: bool,
    },
    Snippet {
        text: String,
    },
    TouchedPath {
        path: PathBuf,
    },
}

impl AgentMenu {
    pub(crate) fn new(
        items: Vec<AgentMenuItem>,
        selected_thread_id: Option<ThreadId>,
        app_event_tx: AppEventSender,
    ) -> Self {
        let items = items
            .into_iter()
            .map(|item| MenuItem {
                label: item.label,
                kind: MenuItemKind::Agent {
                    thread_id: item.thread_id,
                    is_closed: item.is_closed,
                },
            })
            .collect::<Vec<_>>();
        let mut state = ScrollState::new();
        state.selected_idx = selected_thread_id.and_then(|selected_thread_id| {
            items.iter().position(|item| match &item.kind {
                MenuItemKind::Agent { thread_id, .. } => *thread_id == selected_thread_id,
                MenuItemKind::Snippet { .. } | MenuItemKind::TouchedPath { .. } => false,
            })
        });
        state.clamp_selection(items.len());
        state.ensure_visible(items.len(), items.len().max(1));
        Self {
            items,
            state,
            app_event_tx,
            empty_label: "No agents",
            snippet_response_offset: None,
        }
    }

    pub(crate) fn new_snippets(
        snippets: Vec<String>,
        response_offset: usize,
        app_event_tx: AppEventSender,
    ) -> Self {
        let items = snippets
            .into_iter()
            .map(|text| MenuItem {
                label: text.clone(),
                kind: MenuItemKind::Snippet { text },
            })
            .collect::<Vec<_>>();
        let mut state = ScrollState::new();
        state.clamp_selection(items.len());
        state.ensure_visible(items.len(), items.len().max(1));
        Self {
            items,
            state,
            app_event_tx,
            empty_label: "No snippets",
            snippet_response_offset: Some(response_offset),
        }
    }

    pub(crate) fn next_snippet_response_offset(&self) -> Option<usize> {
        self.snippet_response_offset
            .map(|response_offset| response_offset.saturating_add(1))
    }

    pub(crate) fn new_touched_paths(
        paths: Vec<(String, PathBuf)>,
        app_event_tx: AppEventSender,
    ) -> Self {
        let items = paths
            .into_iter()
            .map(|(label, path)| MenuItem {
                label,
                kind: MenuItemKind::TouchedPath { path },
            })
            .collect::<Vec<_>>();
        let mut state = ScrollState::new();
        state.clamp_selection(items.len());
        state.ensure_visible(items.len(), items.len().max(1));
        Self {
            items,
            state,
            app_event_tx,
            empty_label: "No touched paths",
            snippet_response_offset: None,
        }
    }

    pub(crate) fn handle_key_event(&mut self, key_event: KeyEvent) -> bool {
        if !matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return false;
        }

        match key_event.code {
            KeyCode::Esc => true,
            KeyCode::Up => {
                self.state.move_up_wrap(self.items.len());
                self.state
                    .ensure_visible(self.items.len(), self.items.len().max(1));
                false
            }
            KeyCode::Down => {
                self.state.move_down_wrap(self.items.len());
                self.state
                    .ensure_visible(self.items.len(), self.items.len().max(1));
                false
            }
            KeyCode::Enter => {
                if let Some(item) = self.selected_item() {
                    match &item.kind {
                        MenuItemKind::Agent { thread_id, .. } => {
                            self.app_event_tx
                                .send(AppEvent::SelectAgentThread(*thread_id));
                        }
                        MenuItemKind::Snippet { text } => {
                            self.app_event_tx.send(AppEvent::CopySnippet(text.clone()));
                        }
                        MenuItemKind::TouchedPath { path } => {
                            self.app_event_tx
                                .send(AppEvent::OpenTouchedPathInEditor { path: path.clone() });
                        }
                    }
                }
                true
            }
            _ => false,
        }
    }

    pub(crate) fn render(&self, area: Rect, buf: &mut Buffer) {
        let menu_area = self.menu_area(area);
        if menu_area.is_empty() {
            return;
        }
        Clear.render(menu_area, buf);
        let surface = Block::default()
            .borders(Borders::ALL)
            .style(user_message_style());
        let surface_inner = surface.inner(menu_area);
        let inner = Rect::new(
            surface_inner.x.saturating_add(1),
            surface_inner.y,
            surface_inner.width.saturating_sub(2),
            surface_inner.height,
        );
        surface.render(menu_area, buf);
        let rows = self.rows();
        render_rows_single_line(
            inner,
            buf,
            &rows,
            &self.state,
            self.items.len().max(1),
            self.empty_label,
        );
    }

    pub(crate) fn preferred_overlay_height(&self) -> u16 {
        self.menu_height().saturating_add(AGENT_MENU_FOOTER_GAP)
    }

    fn selected_item(&self) -> Option<&MenuItem> {
        self.state
            .selected_idx
            .and_then(|selected_idx| self.items.get(selected_idx))
    }

    fn rows(&self) -> Vec<GenericDisplayRow> {
        self.items
            .iter()
            .map(|item| {
                let name_prefix_spans = match &item.kind {
                    MenuItemKind::Agent { is_closed, .. } => {
                        agent_picker_status_dot_spans(*is_closed)
                    }
                    MenuItemKind::Snippet { .. } | MenuItemKind::TouchedPath { .. } => Vec::new(),
                };
                GenericDisplayRow {
                    name: item.label.clone(),
                    name_prefix_spans,
                    ..Default::default()
                }
            })
            .collect()
    }

    fn menu_area(&self, area: Rect) -> Rect {
        if area.is_empty() {
            return Rect::default();
        }

        let content_width = self
            .items
            .iter()
            .map(|item| {
                let prefix_width = match &item.kind {
                    MenuItemKind::Agent { .. } => 2,
                    MenuItemKind::Snippet { .. } | MenuItemKind::TouchedPath { .. } => 0,
                };
                item.label.chars().count() as u16 + prefix_width
            })
            .max()
            .unwrap_or(0);
        let width = content_width
            .saturating_add(/*menu padding*/ 4)
            .clamp(AGENT_MENU_MIN_WIDTH, area.width.max(AGENT_MENU_MIN_WIDTH))
            .min(area.width);
        let height = self.menu_height().min(area.height);
        let x = area.x;
        let y = area
            .bottom()
            .saturating_sub(height.saturating_add(AGENT_MENU_FOOTER_GAP))
            .max(area.y);
        Rect::new(x, y, width, height)
    }

    fn menu_height(&self) -> u16 {
        let rows = self.items.len().max(1).min(u16::MAX as usize) as u16;
        rows.saturating_add(AGENT_MENU_BORDER_HEIGHT)
    }
}
