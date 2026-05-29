use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use crate::key_hint;
use crate::render::renderable::Renderable;
use crate::wrapping::RtOptions;
use crate::wrapping::adaptive_wrap_lines;

/// Widget that displays pending steers plus follow-up inputs held while a turn is in progress.
///
/// The widget renders pending steers first, then rejected steers that will be
/// resubmitted at end of turn, then ordinary queued user messages. Pending
/// steers explain that they will be submitted after the next tool/result
/// boundary unless the user presses Esc to interrupt and send them
/// immediately. The edit/steer hints at the bottom only appear when there are
/// actual queued user inputs to act on. Because some terminals intercept
/// certain modifier-key combinations, the displayed edit binding is
/// configurable via [`set_edit_binding`](Self::set_edit_binding); the steer
/// binding likewise follows [`set_steer_binding`](Self::set_steer_binding).
pub(crate) struct PendingInputPreview {
    pub pending_steers: Vec<String>,
    pub rejected_steers: Vec<String>,
    pub queued_messages: Vec<String>,
    /// Key combination rendered in the hint line.  Defaults to Alt+Up but may
    /// be overridden for terminals where that chord is unavailable.
    edit_binding: Option<key_hint::KeyBinding>,
    /// Key combination rendered for discarding the last queued message.
    discard_binding: Option<key_hint::KeyBinding>,
    /// Key combination rendered for promoting the last queued message into an
    /// immediate steer. Defaults to Alt+Down but follows the active keymap.
    steer_binding: Option<key_hint::KeyBinding>,
    /// Key combination rendered for immediately interrupting and sending steers.
    interrupt_binding: Option<key_hint::KeyBinding>,
}

const PREVIEW_LINE_LIMIT: usize = 3;

fn compact_binding_label(binding: key_hint::KeyBinding) -> String {
    let (key, modifiers) = binding.parts();
    let mut label = String::new();
    if modifiers.contains(KeyModifiers::CONTROL) {
        label.push('^');
    }
    if modifiers.contains(KeyModifiers::SHIFT) {
        label.push('⇧');
    }
    if modifiers.contains(KeyModifiers::ALT) {
        label.push('⌥');
    }
    let key = match key {
        KeyCode::Char(ch) if modifiers.contains(KeyModifiers::CONTROL) => {
            ch.to_ascii_uppercase().to_string()
        }
        KeyCode::Char(ch) => ch.to_string(),
        KeyCode::Up => "↑".to_string(),
        KeyCode::Down => "↓".to_string(),
        KeyCode::Left => "←".to_string(),
        KeyCode::Right => "→".to_string(),
        KeyCode::Enter => "↵".to_string(),
        _ => key.to_string().to_ascii_lowercase(),
    };
    label.push_str(&key);
    label
}

impl PendingInputPreview {
    pub(crate) fn new() -> Self {
        Self {
            pending_steers: Vec::new(),
            rejected_steers: Vec::new(),
            queued_messages: Vec::new(),
            edit_binding: Some(key_hint::alt(KeyCode::Up)),
            discard_binding: Some(key_hint::ctrl(KeyCode::Char('x'))),
            steer_binding: Some(key_hint::alt(KeyCode::Down)),
            interrupt_binding: Some(key_hint::plain(KeyCode::Esc)),
        }
    }

    /// Replace the keybinding shown in the hint line at the bottom of the
    /// queued-messages list.  The caller is responsible for also wiring the
    /// corresponding key event handler.
    pub(crate) fn set_edit_binding(&mut self, binding: Option<key_hint::KeyBinding>) {
        self.edit_binding = binding;
    }

    /// Replace the keybinding shown for discarding the most recent queued
    /// message. The caller is responsible for wiring the handler.
    pub(crate) fn set_discard_binding(&mut self, binding: Option<key_hint::KeyBinding>) {
        self.discard_binding = binding;
    }

    /// Replace the keybinding shown for steering the most recent queued
    /// message immediately. The caller is responsible for wiring the handler.
    pub(crate) fn set_steer_binding(&mut self, binding: Option<key_hint::KeyBinding>) {
        self.steer_binding = binding;
    }

    pub(crate) fn set_interrupt_binding(&mut self, binding: Option<key_hint::KeyBinding>) {
        self.interrupt_binding = binding;
    }

    fn push_truncated_preview_lines(
        lines: &mut Vec<Line<'static>>,
        wrapped: Vec<Line<'static>>,
        overflow_line: Line<'static>,
    ) {
        let wrapped_len = wrapped.len();
        lines.extend(wrapped.into_iter().take(PREVIEW_LINE_LIMIT));
        if wrapped_len > PREVIEW_LINE_LIMIT {
            lines.push(overflow_line);
        }
    }

    fn push_section_header(lines: &mut Vec<Line<'static>>, width: u16, header: Line<'static>) {
        let mut spans = vec!["• ".dim()];
        spans.extend(header.spans);
        lines.extend(adaptive_wrap_lines(
            std::iter::once(Line::from(spans)),
            RtOptions::new(width as usize).subsequent_indent(Line::from("  ".dim())),
        ));
    }

    fn as_renderable(&self, width: u16) -> Box<dyn Renderable> {
        if (self.pending_steers.is_empty()
            && self.rejected_steers.is_empty()
            && self.queued_messages.is_empty())
            || width < 4
        {
            return Box::new(());
        }

        let mut lines = vec![];

        if !self.pending_steers.is_empty() {
            let mut header = vec!["Messages to be submitted after next tool call".into()];
            if let Some(interrupt_binding) = self.interrupt_binding {
                header.extend(vec![
                    " (press ".dim(),
                    interrupt_binding.into(),
                    " to interrupt and send immediately)".dim(),
                ]);
            }
            Self::push_section_header(&mut lines, width, Line::from(header));

            for steer in &self.pending_steers {
                let wrapped = adaptive_wrap_lines(
                    steer.lines().map(|line| Line::from(line.dim())),
                    RtOptions::new(width as usize)
                        .initial_indent(Line::from("  ↳ ".dim()))
                        .subsequent_indent(Line::from("    ")),
                );
                Self::push_truncated_preview_lines(&mut lines, wrapped, Line::from("    …".dim()));
            }
        }

        if !self.rejected_steers.is_empty() {
            if !lines.is_empty() {
                lines.push(Line::from(""));
            }
            Self::push_section_header(
                &mut lines,
                width,
                "Messages to be submitted at end of turn".into(),
            );

            for steer in &self.rejected_steers {
                let wrapped = adaptive_wrap_lines(
                    steer.lines().map(|line| Line::from(line.dim())),
                    RtOptions::new(width as usize)
                        .initial_indent(Line::from("  ↳ ".dim()))
                        .subsequent_indent(Line::from("    ")),
                );
                Self::push_truncated_preview_lines(&mut lines, wrapped, Line::from("    …".dim()));
            }
        }

        if !self.queued_messages.is_empty() {
            if !lines.is_empty() {
                lines.push(Line::from(""));
            }
            Self::push_section_header(&mut lines, width, "Queued follow-up inputs".into());

            for message in &self.queued_messages {
                let wrapped = adaptive_wrap_lines(
                    message.lines().map(|line| Line::from(line.dim().italic())),
                    RtOptions::new(width as usize)
                        .initial_indent(Line::from("  ↳ ".dim()))
                        .subsequent_indent(Line::from("    ")),
                );
                Self::push_truncated_preview_lines(
                    &mut lines,
                    wrapped,
                    Line::from("    …".dim().italic()),
                );
            }
        }

        if !self.queued_messages.is_empty() {
            let actions = [
                self.edit_binding.map(|binding| (binding, "edit")),
                self.discard_binding.map(|binding| (binding, "drop")),
                self.steer_binding.map(|binding| (binding, "steer")),
            ];
            let mut spans = vec!["    ".into()];
            for (idx, (binding, label)) in actions.into_iter().flatten().enumerate() {
                if idx > 0 {
                    spans.push(" · ".into());
                }
                spans.push(format!("{} {label}", compact_binding_label(binding)).into());
            }
            if spans.len() > 1 {
                lines.push(Line::from(spans).dim());
            }
        }

        Paragraph::new(lines).into()
    }
}

impl Renderable for PendingInputPreview {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }

        self.as_renderable(area.width).render(area, buf);
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.as_renderable(width).desired_height(width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;
    use pretty_assertions::assert_eq;

    #[test]
    fn desired_height_empty() {
        let queue = PendingInputPreview::new();
        assert_eq!(queue.desired_height(/*width*/ 40), 0);
    }

    #[test]
    fn desired_height_one_message() {
        let mut queue = PendingInputPreview::new();
        queue.queued_messages.push("Hello, world!".to_string());
        assert_eq!(queue.desired_height(/*width*/ 40), 3);
    }

    #[test]
    fn render_one_message() {
        let mut queue = PendingInputPreview::new();
        queue.queued_messages.push("Hello, world!".to_string());
        let width = 40;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_one_message", format!("{buf:?}"));
    }

    #[test]
    fn render_one_message_with_shift_left_binding() {
        let mut queue = PendingInputPreview::new();
        queue.queued_messages.push("Hello, world!".to_string());
        queue.set_edit_binding(Some(key_hint::shift(KeyCode::Left)));
        let width = 40;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!(
            "render_one_message_with_shift_left_binding",
            format!("{buf:?}")
        );
    }

    #[test]
    fn render_two_messages() {
        let mut queue = PendingInputPreview::new();
        queue.queued_messages.push("Hello, world!".to_string());
        queue
            .queued_messages
            .push("This is another message".to_string());
        let width = 40;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_two_messages", format!("{buf:?}"));
    }

    #[test]
    fn render_more_than_three_messages() {
        let mut queue = PendingInputPreview::new();
        queue.queued_messages.push("Hello, world!".to_string());
        queue
            .queued_messages
            .push("This is another message".to_string());
        queue
            .queued_messages
            .push("This is a third message".to_string());
        queue
            .queued_messages
            .push("This is a fourth message".to_string());
        let width = 40;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_more_than_three_messages", format!("{buf:?}"));
    }

    #[test]
    fn render_wrapped_message() {
        let mut queue = PendingInputPreview::new();
        queue
            .queued_messages
            .push("This is a longer message that should be wrapped".to_string());
        queue
            .queued_messages
            .push("This is another message".to_string());
        let width = 40;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_wrapped_message", format!("{buf:?}"));
    }

    #[test]
    fn render_many_line_message() {
        let mut queue = PendingInputPreview::new();
        queue
            .queued_messages
            .push("This is\na message\nwith many\nlines".to_string());
        let width = 40;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_many_line_message", format!("{buf:?}"));
    }

    #[test]
    fn long_url_like_message_does_not_expand_into_wrapped_ellipsis_rows() {
        let mut queue = PendingInputPreview::new();
        queue.queued_messages.push(
            "example.test/api/v1/projects/alpha-team/releases/2026-02-17/builds/1234567890/artifacts/reports/performance/summary/detail/session_id=abc123def456ghi789"
                .to_string(),
        );

        let width = 36;
        let height = queue.desired_height(width);
        assert_eq!(
            height, 3,
            "expected header, one message row, and one controls row for URL-like token"
        );

        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);

        let rendered_rows = (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buf[(x, y)].symbol().chars().next().unwrap_or(' '))
                    .collect::<String>()
            })
            .collect::<Vec<_>>();

        assert!(
            !rendered_rows.iter().any(|row| row.contains('…')),
            "expected no wrapped-ellipsis row for URL-like token, got rows: {rendered_rows:?}"
        );
    }

    #[test]
    fn render_one_pending_steer() {
        let mut queue = PendingInputPreview::new();
        queue.pending_steers.push("Please continue.".to_string());
        let width = 48;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!("render_one_pending_steer", format!("{buf:?}"));
    }

    #[test]
    fn render_one_pending_steer_with_remapped_interrupt_binding() {
        let mut queue = PendingInputPreview::new();
        queue.pending_steers.push("Please continue.".to_string());
        queue.set_interrupt_binding(Some(key_hint::plain(KeyCode::F(12))));
        let width = 48;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!(
            "render_one_pending_steer_with_remapped_interrupt_binding",
            format!("{buf:?}")
        );
    }

    #[test]
    fn render_pending_steers_above_queued_messages() {
        let mut queue = PendingInputPreview::new();
        queue.pending_steers.push("Please continue.".to_string());
        queue
            .pending_steers
            .push("Check the last command output.".to_string());
        queue
            .rejected_steers
            .push("Rejected steer that will be retried.".to_string());
        queue
            .queued_messages
            .push("Queued follow-up question".to_string());
        let width = 52;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!(
            "render_pending_steers_above_queued_messages",
            format!("{buf:?}")
        );
    }

    #[test]
    fn render_multiline_pending_steer_uses_single_prefix_and_truncates() {
        let mut queue = PendingInputPreview::new();
        queue
            .pending_steers
            .push("First line\nSecond line\nThird line\nFourth line".to_string());
        let width = 48;
        let height = queue.desired_height(width);
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        queue.render(Rect::new(0, 0, width, height), &mut buf);
        assert_snapshot!(
            "render_multiline_pending_steer_uses_single_prefix_and_truncates",
            format!("{buf:?}")
        );
    }
}
