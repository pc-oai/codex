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
use crate::style::edited_user_message_style;
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
    items: Vec<AgentMenuItem>,
    state: ScrollState,
    app_event_tx: AppEventSender,
}

impl AgentMenu {
    pub(crate) fn new(
        items: Vec<AgentMenuItem>,
        selected_thread_id: Option<ThreadId>,
        app_event_tx: AppEventSender,
    ) -> Self {
        let mut state = ScrollState::new();
        state.selected_idx = selected_thread_id.and_then(|selected_thread_id| {
            items
                .iter()
                .position(|item| item.thread_id == selected_thread_id)
        });
        state.clamp_selection(items.len());
        state.ensure_visible(items.len(), items.len().max(1));
        Self {
            items,
            state,
            app_event_tx,
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
                    self.app_event_tx
                        .send(AppEvent::SelectAgentThread(item.thread_id));
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
        self.render_alternating_row_surfaces(surface_inner, buf);
        let rows = self.rows();
        render_rows_single_line(
            inner,
            buf,
            &rows,
            &self.state,
            self.items.len().max(1),
            "No agents",
        );
    }

    pub(crate) fn preferred_overlay_height(&self) -> u16 {
        self.menu_height().saturating_add(AGENT_MENU_FOOTER_GAP)
    }

    fn render_alternating_row_surfaces(&self, area: Rect, buf: &mut Buffer) {
        let visible_items = self.items.len().min(area.height as usize);
        let start_idx = self.visible_start_idx(visible_items);
        for (visible_offset, item_idx) in (start_idx..self.items.len())
            .take(visible_items)
            .enumerate()
        {
            if item_idx % 2 == 0 {
                continue;
            }
            Block::default().style(edited_user_message_style()).render(
                Rect::new(
                    area.x,
                    area.y.saturating_add(visible_offset as u16),
                    area.width,
                    1,
                ),
                buf,
            );
        }
    }

    fn visible_start_idx(&self, visible_items: usize) -> usize {
        if self.items.is_empty() {
            return 0;
        }

        let mut start_idx = self
            .state
            .scroll_top
            .min(self.items.len().saturating_sub(1));
        if let Some(selected_idx) = self.state.selected_idx {
            if selected_idx < start_idx {
                start_idx = selected_idx;
            } else if visible_items > 0 {
                let bottom = start_idx + visible_items - 1;
                if selected_idx > bottom {
                    start_idx = selected_idx + 1 - visible_items;
                }
            }
        }
        start_idx
    }

    fn selected_item(&self) -> Option<&AgentMenuItem> {
        self.state
            .selected_idx
            .and_then(|selected_idx| self.items.get(selected_idx))
    }

    fn rows(&self) -> Vec<GenericDisplayRow> {
        self.items
            .iter()
            .map(|item| GenericDisplayRow {
                name: item.label.clone(),
                name_prefix_spans: agent_picker_status_dot_spans(item.is_closed),
                ..Default::default()
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
            .map(|item| item.label.chars().count() as u16 + /*status dot*/ 2)
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
