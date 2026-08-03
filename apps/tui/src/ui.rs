use luna_protocol::{MessageDelivery, MessageRole, MessageStatus, SessionState};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use unicode_width::UnicodeWidthChar;

use crate::{
    app::{App, ConnectionStatus, Focus},
    input::sanitize_terminal_text,
};

const BASE: Color = Color::Rgb(30, 30, 46);
const SURFACE: Color = Color::Rgb(49, 50, 68);
const TEXT: Color = Color::Rgb(205, 214, 244);
const SUBTEXT: Color = Color::Rgb(166, 173, 200);
const BLUE: Color = Color::Rgb(137, 180, 250);
const GREEN: Color = Color::Rgb(166, 227, 161);
const YELLOW: Color = Color::Rgb(249, 226, 175);
const RED: Color = Color::Rgb(243, 139, 168);
const MAUVE: Color = Color::Rgb(203, 166, 247);

pub fn render(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    frame.render_widget(Block::default().style(style(app, TEXT, BASE)), area);
    if area.width < 50 || area.height < 16 {
        frame.render_widget(
            Paragraph::new("Luna needs at least 50 columns by 16 rows.\nResize the terminal, or press Ctrl-C to quit.")
                .alignment(Alignment::Center)
                .style(style(app, YELLOW, BASE))
                .wrap(Wrap { trim: false }),
            area,
        );
        return;
    }

    if area.width >= 90 {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(31), Constraint::Min(40)])
            .split(area);
        render_conversations(frame, app, columns[0]);
        render_conversation(frame, app, columns[1]);
    } else if app.focus == Focus::List {
        render_conversations(frame, app, area);
    } else {
        render_conversation(frame, app, area);
    }

    if app.show_help {
        render_help(frame, app, centered(area, 62, 18));
    } else if app.confirm_interrupt {
        render_confirmation(frame, app, centered(area, 52, 7));
    }
}

fn render_conversations(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let active = app.focus == Focus::List;
    let border = if active { BLUE } else { SURFACE };
    let items = if app.state.conversations.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "No conversations — press n",
            style(app, SUBTEXT, BASE),
        )))]
    } else {
        app.state
            .conversations
            .iter()
            .map(|conversation| {
                let title = sanitize_terminal_text(&conversation.title);
                let state = session_state(conversation.state);
                ListItem::new(vec![
                    Line::from(Span::styled(
                        title,
                        style(app, TEXT, BASE).add_modifier(Modifier::BOLD),
                    )),
                    Line::from(vec![
                        Span::styled(state, style(app, state_color(conversation.state), BASE)),
                        Span::styled(
                            format!("  {}", compact_path(&conversation.active_working_directory)),
                            style(app, SUBTEXT, BASE),
                        ),
                    ]),
                ])
            })
            .collect()
    };
    let list = List::new(items)
        .block(
            Block::default()
                .title(" Luna ")
                .title_bottom(Line::from(" n new · Enter open · ? help ").centered())
                .borders(Borders::ALL)
                .border_style(style(app, border, BASE)),
        )
        .highlight_symbol("› ")
        .highlight_style(style(app, BASE, BLUE).add_modifier(Modifier::BOLD));
    let mut list_state = ListState::default();
    if !app.state.conversations.is_empty() {
        list_state.select(Some(
            app.list_index
                .min(app.state.conversations.len().saturating_sub(1)),
        ));
    }
    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_conversation(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(5),
            Constraint::Length(5),
            Constraint::Length(1),
        ])
        .split(area);
    render_header(frame, app, rows[0]);
    render_transcript(frame, app, rows[1]);
    render_composer(frame, app, rows[2]);
    render_footer(frame, app, rows[3]);
}

fn render_header(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let Some(conversation) = app.state.selected_conversation() else {
        frame.render_widget(
            Paragraph::new("No conversation selected")
                .block(Block::default().borders(Borders::ALL))
                .style(style(app, SUBTEXT, BASE)),
            area,
        );
        return;
    };
    let mut lines = vec![Line::from(vec![
        Span::styled(
            sanitize_terminal_text(&conversation.title),
            style(app, TEXT, BASE).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", style(app, TEXT, BASE)),
        Span::styled(
            session_state(conversation.state),
            style(app, state_color(conversation.state), BASE),
        ),
    ])];
    let repository = conversation
        .repositories
        .iter()
        .find(|repository| repository.active)
        .or_else(|| conversation.repositories.first())
        .map(|repository| {
            let branch = repository
                .branch
                .as_deref()
                .map(|branch| format!(" · {branch}"))
                .unwrap_or_default();
            format!("{}{}", repository.display_name, branch)
        })
        .unwrap_or_else(|| compact_path(&conversation.active_working_directory));
    lines.push(Line::from(Span::styled(
        sanitize_terminal_text(&repository),
        style(app, SUBTEXT, BASE),
    )));
    if let Some(activity) = conversation.activities.last() {
        lines.push(Line::from(Span::styled(
            sanitize_terminal_text(&activity.summary),
            style(app, MAUVE, BASE),
        )));
    }
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(style(app, SURFACE, BASE)),
        ),
        area,
    );
}

fn render_transcript(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let active = app.focus == Focus::Transcript;
    let block = Block::default()
        .title(" Transcript ")
        .borders(Borders::ALL)
        .border_style(style(app, if active { BLUE } else { SURFACE }, BASE));
    let inner = block.inner(area);
    let lines = transcript_lines(app);
    let paragraph = Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false });
    let line_count = paragraph.line_count(inner.width).min(usize::from(u16::MAX)) as u16;
    let maximum_scroll = line_count.saturating_sub(inner.height);
    let scroll = maximum_scroll.saturating_sub(app.transcript_offset_from_bottom);
    frame.render_widget(paragraph.scroll((scroll, 0)).block(block), area);
}

fn transcript_lines(app: &App) -> Vec<Line<'static>> {
    if app.state.selected_conversation_id.is_none() {
        return vec![Line::from(Span::styled(
            "Open or create a conversation.",
            style(app, SUBTEXT, BASE),
        ))];
    }
    if app.state.selected_messages().is_empty() {
        return vec![Line::from(Span::styled(
            if app.pending_action {
                "Loading messages…"
            } else {
                "No messages yet. Press i to compose."
            },
            style(app, SUBTEXT, BASE),
        ))];
    }
    let mut lines = Vec::new();
    for message in app.state.selected_messages() {
        let (label, color) = match message.role {
            MessageRole::User => ("YOU", BLUE),
            MessageRole::Assistant => ("LUNA", MAUVE),
        };
        let delivery = match message.delivery {
            Some(MessageDelivery::Steer) => " · steer",
            Some(MessageDelivery::Bash) => " · shell",
            _ => "",
        };
        let status = match message.status {
            MessageStatus::Streaming => " · streaming",
            MessageStatus::Interrupted => " · interrupted",
            MessageStatus::Failed => " · failed",
            MessageStatus::Queued => " · queued",
            _ => "",
        };
        lines.push(Line::from(Span::styled(
            format!("{label}{delivery}{status}"),
            style(app, color, BASE).add_modifier(Modifier::BOLD),
        )));
        let text = sanitize_terminal_text(&message.text);
        if text.is_empty() {
            lines.push(Line::from(Span::styled("…", style(app, SUBTEXT, BASE))));
        } else {
            lines.extend(
                text.split('\n')
                    .map(|line| Line::from(Span::styled(line.to_owned(), style(app, TEXT, BASE)))),
            );
        }
        for attachment in &message.attachments {
            lines.push(Line::from(Span::styled(
                format!(
                    "[attachment: {}]",
                    sanitize_terminal_text(&attachment.file_name)
                ),
                style(app, YELLOW, BASE),
            )));
        }
        lines.push(Line::default());
    }
    lines
}

fn render_composer(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let active = app.focus == Focus::Composer;
    let block = Block::default()
        .title(if app.pending_action {
            " Composer · sending "
        } else {
            " Composer · Enter send · Alt-Enter newline "
        })
        .borders(Borders::ALL)
        .border_style(style(app, if active { BLUE } else { SURFACE }, BASE));
    let inner = block.inner(area);
    let (cursor_row, cursor_column) = composer_cursor(app, inner.width.max(1));
    let scroll = cursor_row.saturating_sub(inner.height.saturating_sub(1));
    let content = if app.composer.text().is_empty() && !active {
        Text::from(Line::from(Span::styled(
            "Press i or Tab to compose",
            style(app, SUBTEXT, BASE),
        )))
    } else {
        Text::from(sanitize_terminal_text(app.composer.text()))
    };
    frame.render_widget(
        Paragraph::new(content)
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0))
            .block(block),
        area,
    );
    if active && !app.show_help && !app.confirm_interrupt && !app.pending_action {
        frame.set_cursor_position((
            inner.x + cursor_column.min(inner.width.saturating_sub(1)),
            inner.y
                + cursor_row
                    .saturating_sub(scroll)
                    .min(inner.height.saturating_sub(1)),
        ));
    }
}

fn composer_cursor(app: &App, width: u16) -> (u16, u16) {
    let mut row = 0_u16;
    let mut column = 0_u16;
    for character in app.composer.text()[..app.composer.cursor()].chars() {
        if character == '\n' {
            row = row.saturating_add(1);
            column = 0;
            continue;
        }
        let character_width = character.width().unwrap_or(0).min(usize::from(u16::MAX)) as u16;
        if column.saturating_add(character_width) > width {
            row = row.saturating_add(1);
            column = 0;
        }
        column = column.saturating_add(character_width);
        if column >= width {
            row = row.saturating_add(1);
            column = 0;
        }
    }
    (row, column)
}

fn render_footer(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let connection = match app.connection {
        ConnectionStatus::Connecting => ("connecting", YELLOW),
        ConnectionStatus::Connected => ("connected", GREEN),
        ConnectionStatus::Waiting => ("reconnecting", RED),
    };
    let message = app
        .error
        .as_deref()
        .map(|error| (sanitize_terminal_text(error), RED))
        .or_else(|| {
            app.notice
                .as_deref()
                .map(|notice| (sanitize_terminal_text(notice), GREEN))
        })
        .unwrap_or_else(|| {
            (
                "Tab focus · ↑/↓ scroll · PageUp history · s stop · q quit".into(),
                SUBTEXT,
            )
        });
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!(" {} ", connection.0),
                style(app, BASE, connection.1),
            ),
            Span::styled(format!(" {}", message.0), style(app, message.1, BASE)),
        ])),
        area,
    );
}

fn render_help(frame: &mut Frame<'_>, app: &App, area: Rect) {
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "Luna TUI",
                style(app, MAUVE, BASE).add_modifier(Modifier::BOLD),
            )),
            Line::default(),
            Line::from("Tab              cycle focus"),
            Line::from("↑/↓ or j/k       navigate and scroll"),
            Line::from("Enter            open conversation / send message"),
            Line::from("Alt-Enter        insert newline"),
            Line::from("i                focus composer"),
            Line::from("n                create conversation"),
            Line::from("s                interrupt active work"),
            Line::from("PageUp           load earlier messages"),
            Line::from("End              return to live output"),
            Line::from("q or Ctrl-C      quit without interrupting Pi"),
            Line::default(),
            Line::from(Span::styled(
                "Press ? or Esc to close",
                style(app, SUBTEXT, BASE),
            )),
        ])
        .block(
            Block::default()
                .title(" Help ")
                .borders(Borders::ALL)
                .border_style(style(app, BLUE, BASE)),
        )
        .style(style(app, TEXT, BASE)),
        area,
    );
}

fn render_confirmation(frame: &mut Frame<'_>, app: &App, area: Rect) {
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from("Interrupt the current Pi operation?"),
            Line::default(),
            Line::from(Span::styled(
                "Enter/y confirm · n/Esc cancel",
                style(app, SUBTEXT, BASE),
            )),
        ])
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .title(" Confirm interrupt ")
                .borders(Borders::ALL)
                .border_style(style(app, RED, BASE)),
        )
        .style(style(app, TEXT, BASE)),
        area,
    );
}

fn style(app: &App, foreground: Color, background: Color) -> Style {
    if app.color {
        Style::default().fg(foreground).bg(background)
    } else {
        Style::default()
    }
}

fn compact_path(value: &str) -> String {
    let clean = sanitize_terminal_text(value);
    let component = clean
        .rsplit('/')
        .find(|component| !component.is_empty())
        .map(str::to_owned);
    component.unwrap_or(clean)
}

const fn session_state(state: SessionState) -> &'static str {
    match state {
        SessionState::Creating => "creating",
        SessionState::Starting => "starting",
        SessionState::Idle => "idle",
        SessionState::Working => "working",
        SessionState::Compacting => "compacting",
        SessionState::Retrying => "retrying",
        SessionState::Crashed => "crashed",
        SessionState::Restoring => "restoring",
        SessionState::Interrupted => "interrupted",
        SessionState::Stopped => "stopped",
        SessionState::Error => "error",
    }
}

const fn state_color(state: SessionState) -> Color {
    match state {
        SessionState::Working
        | SessionState::Compacting
        | SessionState::Retrying
        | SessionState::Starting
        | SessionState::Restoring => YELLOW,
        SessionState::Idle => GREEN,
        SessionState::Crashed | SessionState::Error => RED,
        SessionState::Creating | SessionState::Interrupted | SessionState::Stopped => SUBTEXT,
    }
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width.saturating_sub(2));
    let height = height.min(area.height.saturating_sub(2));
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    )
}

#[cfg(test)]
mod tests {
    use luna_protocol::Bootstrap;
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;
    use crate::app::App;

    #[test]
    fn renders_wide_and_narrow_layouts() {
        let app = App::new(bootstrap());
        for (width, height) in [(120, 36), (70, 24)] {
            let backend = TestBackend::new(width, height);
            let mut terminal = Terminal::new(backend).expect("terminal");
            terminal.draw(|frame| render(frame, &app)).expect("draw");
            let rendered = buffer_text(terminal.backend().buffer());
            if width >= 90 {
                assert!(rendered.contains("Luna"));
            }
            assert!(rendered.contains("Conversation"));
            assert!(rendered.contains("connected") || rendered.contains("connecting"));
        }
    }

    #[test]
    fn keeps_the_latest_wrapped_output_visible() {
        let mut app = App::new(bootstrap());
        let conversation_id = app.state.selected_conversation_id.expect("selection");
        app.state.set_messages(
            conversation_id,
            serde_json::from_value(serde_json::json!({
                "messages": [{
                    "id": "00000000-0000-0000-0000-000000000003",
                    "conversationId": conversation_id,
                    "role": "assistant",
                    "status": "completed",
                    "text": format!("{}LATEST OUTPUT", "wrapped words ".repeat(200)),
                    "attachments": [],
                    "ordinal": 1,
                    "createdAt": "2026-01-01T00:00:01Z",
                    "updatedAt": "2026-01-01T00:00:01Z"
                }]
            }))
            .expect("messages"),
        );
        let backend = TestBackend::new(70, 24);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal.draw(|frame| render(frame, &app)).expect("draw");

        assert!(buffer_text(terminal.backend().buffer()).contains("LATEST OUTPUT"));
    }

    #[test]
    fn does_not_render_terminal_escape_characters() {
        let mut bootstrap = bootstrap();
        bootstrap.conversations[0].title = "Unsafe\u{1b}]8;;title\u{7}".into();
        let app = App::new(bootstrap);
        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal.draw(|frame| render(frame, &app)).expect("draw");
        let rendered = buffer_text(terminal.backend().buffer());
        assert!(!rendered.contains('\u{1b}'));
        assert!(!rendered.contains('\u{7}'));
        assert!(rendered.contains("]8;;title"));
    }

    fn buffer_text(buffer: &ratatui::buffer::Buffer) -> String {
        let area = buffer.area;
        (area.y..area.bottom())
            .map(|y| {
                (area.x..area.right())
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn bootstrap() -> Bootstrap {
        serde_json::from_value(serde_json::json!({
            "protocolVersion": 1,
            "cursor": 1,
            "device": {
                "id": "00000000-0000-0000-0000-000000000001",
                "name": "Terminal",
                "platform": "tui",
                "notificationsEnabled": false,
                "createdAt": "2026-01-01T00:00:00Z",
                "lastSeenAt": "2026-01-01T00:00:00Z"
            },
            "conversations": [{
                "id": "00000000-0000-0000-0000-000000000002",
                "title": "Conversation",
                "titleMode": "automatic",
                "state": "idle",
                "preview": "",
                "activeWorkingDirectory": "/tmp/luna",
                "repositories": [],
                "activities": [],
                "unreadCount": 0,
                "createdAt": "2026-01-01T00:00:00Z",
                "updatedAt": "2026-01-01T00:00:00Z",
                "version": 1
            }]
        }))
        .expect("bootstrap")
    }
}
