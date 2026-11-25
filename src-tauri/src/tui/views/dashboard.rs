use crate::core::device_pool::SlotStatus;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

/// Renders the dual-slot (`Slot0` & `Slot1`) status overview in the terminal UI.
pub fn render_dashboard(frame: &mut Frame<'_>, slots: &[SlotStatus]) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(frame.size());

    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            " Kernel Overview & Diagnostics - KOD ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("| Dual-Device Multiplexer (Slot 0 & Slot 1)"),
    ]))
    .block(Block::default().borders(Borders::ALL).title("Status"));
    frame.render_widget(header, chunks[0]);

    let items: Vec<ListItem<'_>> = slots
        .iter()
        .map(|s| {
            let state_text = if s.occupied {
                let id = s.device_id.as_deref().unwrap_or("unknown");
                let plat = s
                    .platform
                    .map(|p| format!("{p:?}"))
                    .unwrap_or_else(|| "Unknown".to_string());
                format!("[{:?}] ACTIVE | Platform: {plat} | ID: {id}", s.slot)
            } else {
                format!("[{:?}] EMPTY  | Awaiting USB device...", s.slot)
            };
            ListItem::new(Line::from(state_text))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Isolated Device Slots"),
    );
    frame.render_widget(list, chunks[1]);

    let footer = Paragraph::new(" Press 'q' or Esc to exit TUI ")
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(footer, chunks[2]);
}
