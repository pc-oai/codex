mod transcript_browser_shared;

use std::io;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::read;
use crossterm::execute;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::List;
use ratatui::widgets::ListItem;
use ratatui::widgets::ListState;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;

use transcript_browser_shared::BrowserData;
use transcript_browser_shared::load_browser_data;

#[derive(Debug, Parser)]
#[command(about = "Split-pane tree transcript prototype")]
struct Cli {
    rollout: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let data = load_browser_data(&cli.rollout)?;
    run(data)
}

fn run(data: BrowserData) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut selected = data.nodes.len().saturating_sub(1);

    let result = loop {
        terminal.draw(|frame| {
            let area = frame.area();
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(44), Constraint::Percentage(56)])
                .split(area);
            let items = data
                .nodes
                .iter()
                .map(|node| {
                    ListItem::new(Line::from(format!(
                        "{}{}",
                        "  ".repeat(node.depth),
                        node.label
                    )))
                })
                .collect::<Vec<_>>();
            let mut state = ListState::default();
            state.select(Some(selected));
            frame.render_stateful_widget(
                List::new(items)
                    .block(
                        Block::default()
                            .title("Transcript Tree")
                            .borders(Borders::ALL),
                    )
                    .highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
                chunks[0],
                &mut state,
            );
            let detail = data
                .nodes
                .get(selected)
                .map(|node| node.detail.as_str())
                .unwrap_or("(no selection)");
            frame.render_widget(
                Paragraph::new(detail)
                    .block(
                        Block::default()
                            .title(format!("Detail - {}", data.rollout_path.display()))
                            .borders(Borders::ALL),
                    )
                    .wrap(Wrap { trim: false }),
                chunks[1],
            );
        })?;

        if let Event::Key(key) = read()? {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break Ok(()),
                KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => {
                    selected = (selected + 1).min(data.nodes.len().saturating_sub(1));
                }
                KeyCode::Home | KeyCode::Char('g') => selected = 0,
                KeyCode::End | KeyCode::Char('G') => {
                    selected = data.nodes.len().saturating_sub(1);
                }
                _ => {}
            }
        }
    };

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    result
}
