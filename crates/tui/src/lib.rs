//! ZenvX TUI: full-screen chat interface with a status bar and app launcher.
//! Rendering is pure (testable with a TestBackend); the event loop lives in the bin.

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Wrap};
use ratatui::Frame;
use zenvx_ai_core::Message;

#[derive(Clone, Copy, PartialEq)]
pub enum Role {
    User,
    Assistant,
    System,
}

pub struct App {
    pub provider: String,
    pub model: String,
    pub status: String,
    pub input: String,
    pub messages: Vec<(Role, String)>,
}

impl App {
    pub fn new(provider: &str, model: &str) -> Self {
        Self {
            provider: provider.into(),
            model: model.into(),
            status: "ready".into(),
            input: String::new(),
            messages: vec![(Role::System, "Welcome to ZenvX OS. Type to chat, /launch <app> to open something, /quit to exit.".into())],
        }
    }

    pub fn push(&mut self, role: Role, text: impl Into<String>) {
        self.messages.push((role, text.into()));
    }

    pub fn start_assistant(&mut self) {
        self.messages.push((Role::Assistant, String::new()));
    }

    pub fn append_assistant(&mut self, tok: &str) {
        if let Some((Role::Assistant, s)) = self.messages.last_mut() {
            s.push_str(tok);
        } else {
            self.messages.push((Role::Assistant, tok.into()));
        }
    }

    /// Conversation as provider messages (UI-only system notices are dropped).
    pub fn to_messages(&self) -> Vec<Message> {
        let mut out = vec![Message::system("You are the ZenvX OS assistant. Be concise.")];
        for (role, text) in &self.messages {
            match role {
                Role::User => out.push(Message::user(text.clone())),
                Role::Assistant if !text.is_empty() => {
                    out.push(Message { role: "assistant".into(), content: text.clone() })
                }
                _ => {}
            }
        }
        out
    }

    fn transcript_lines(&self) -> Vec<Line<'_>> {
        let mut lines = Vec::new();
        for (role, text) in &self.messages {
            let (tag, style) = match role {
                Role::User => ("you", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Role::Assistant => ("ai ", Style::default().fg(Color::Green)),
                Role::System => ("·  ", Style::default().fg(Color::DarkGray)),
            };
            let mut first = true;
            for seg in text.split('\n') {
                let prefix = if first { format!("{tag} ") } else { "    ".into() };
                lines.push(Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled(seg.to_string(), style),
                ]));
                first = false;
            }
        }
        lines
    }
}

/// Render the whole UI for the current state.
pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title bar
            Constraint::Min(1),    // transcript
            Constraint::Length(3), // input
            Constraint::Length(1), // help
        ])
        .split(f.area());

    let title = Line::from(vec![
        Span::styled(
            " ZenvX OS ",
            Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!("  {} · {}", app.provider, app.model)),
        Span::styled(format!("   [{}]", app.status), Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(title), chunks[0]);

    let lines = app.transcript_lines();
    let view_h = chunks[1].height.saturating_sub(2);
    let scroll = (lines.len() as u16).saturating_sub(view_h);
    let body = Paragraph::new(lines)
        .block(Block::bordered().title(" conversation "))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    f.render_widget(body, chunks[1]);

    let input = Paragraph::new(format!("› {}", app.input))
        .block(Block::bordered().title(" message "));
    f.render_widget(input, chunks[2]);

    f.render_widget(
        Paragraph::new(Span::styled(
            " Enter: send    /launch <app>    /quit (or Esc) ",
            Style::default().fg(Color::DarkGray),
        )),
        chunks[3],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn renders_brand_and_widgets() {
        let app = App::new("openrouter", "openrouter/auto");
        let mut term = Terminal::new(TestBackend::new(80, 20)).unwrap();
        term.draw(|f| draw(f, &app)).unwrap();
        let text: String =
            term.backend().buffer().content.iter().map(|c| c.symbol()).collect();
        assert!(text.contains("ZenvX OS"));
        assert!(text.contains("conversation"));
        assert!(text.contains("message"));
    }

    #[test]
    fn assistant_streaming_appends() {
        let mut app = App::new("ollama", "llama3.2:1b");
        app.start_assistant();
        app.append_assistant("Hel");
        app.append_assistant("lo");
        assert_eq!(app.messages.last().unwrap().1, "Hello");
    }
}
