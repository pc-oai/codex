mod transcript_browser_shared;

use std::collections::BTreeSet;
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
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::List;
use ratatui::widgets::ListItem;
use ratatui::widgets::ListState;

use transcript_browser_shared::BrowserData;
use transcript_browser_shared::load_browser_data;
use transcript_browser_shared::preview;

#[derive(Debug, Parser)]
#[command(about = "Single-pane outline transcript prototype")]
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
    let mut expanded = BTreeSet::new();

    let result = loop {
        terminal.draw(|frame| {
            let items = data
                .nodes
                .iter()
                .enumerate()
                .map(|(index, node)| {
                    let marker = if expanded.contains(&index) { "-" } else { "+" };
                    let mut lines = vec![Line::from(format!(
                        "{}{} {}",
                        "  ".repeat(node.depth),
                        marker,
                        node.label
                    ))];
                    if expanded.contains(&index) {
                        lines.push(Line::from(format!(
                            "{}{}",
                            "  ".repeat(node.depth + 1),
                            preview(&node.detail)
                        )));
                    }
                    ListItem::new(lines)
                })
                .collect::<Vec<_>>();
            let mut state = ListState::default();
            state.select(Some(selected));
            frame.render_stateful_widget(
                List::new(items)
                    .block(
                        Block::default()
                            .title(format!(
                                "Transcript Outline - {}",
                                data.rollout_path.display()
                            ))
                            .borders(Borders::ALL),
                    )
                    .highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
                frame.area(),
                &mut state,
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
                KeyCode::Enter | KeyCode::Char(' ') => {
                    if !expanded.insert(selected) {
                        expanded.remove(&selected);
                    }
                }
                _ => {}
            }
        }
    };

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    result
}
