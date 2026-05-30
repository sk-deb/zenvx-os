//! ZenvX TUI event loop.

use std::io::{self, IsTerminal};

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::execute;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use zenvx_ai_core::{from_config, LlmProvider};
use zenvx_common::config::Config;
use zenvx_tui::{draw, App, Role};

fn main() -> io::Result<()> {
    if !io::stdout().is_terminal() {
        eprintln!("zenvx-tui needs an interactive terminal.");
        std::process::exit(1);
    }

    let cfg = Config::load();
    let (provider_name, model) = match &cfg {
        Some(c) => (c.provider.as_str().to_string(), c.model.clone().unwrap_or_default()),
        None => ("unset".into(), String::new()),
    };
    let mut app = App::new(&provider_name, &model);
    let provider: Option<Box<dyn LlmProvider>> = cfg.as_ref().and_then(|c| from_config(c).ok());
    if provider.is_none() {
        app.push(Role::System, "No provider configured — run `zenvx` first to add a key or pick a local model.");
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut term = Terminal::new(CrosstermBackend::new(stdout))?;

    let res = run(&mut term, &mut app, provider.as_deref());

    disable_raw_mode()?;
    execute!(term.backend_mut(), LeaveAlternateScreen)?;
    res
}

fn run<B: ratatui::backend::Backend>(
    term: &mut Terminal<B>,
    app: &mut App,
    provider: Option<&dyn LlmProvider>,
) -> io::Result<()> {
    loop {
        term.draw(|f| draw(f, app))?;
        let Event::Key(key) = event::read()? else { continue };
        if key.kind != event::KeyEventKind::Press {
            continue;
        }
        match (key.code, key.modifiers) {
            (KeyCode::Char('c'), KeyModifiers::CONTROL) | (KeyCode::Esc, _) => break,
            (KeyCode::Enter, _) => {
                let line = std::mem::take(&mut app.input);
                if line.trim() == "/quit" {
                    break;
                }
                handle_input(term, app, provider, line.trim())?;
            }
            (KeyCode::Backspace, _) => {
                app.input.pop();
            }
            (KeyCode::Char(c), _) => app.input.push(c),
            _ => {}
        }
    }
    Ok(())
}

fn handle_input<B: ratatui::backend::Backend>(
    term: &mut Terminal<B>,
    app: &mut App,
    provider: Option<&dyn LlmProvider>,
    line: &str,
) -> io::Result<()> {
    if line.is_empty() {
        return Ok(());
    }

    if let Some(target) = line.strip_prefix("/launch ") {
        app.push(Role::User, line.to_string());
        match zenvx_launcher::launch(target.trim()) {
            Ok(cmd) => app.push(Role::System, format!("launching: {cmd}")),
            Err(e) => app.push(Role::System, format!("launch failed: {e}")),
        }
        return Ok(());
    }

    app.push(Role::User, line.to_string());
    let Some(p) = provider else {
        app.push(Role::System, "No provider configured.");
        return Ok(());
    };

    app.start_assistant();
    app.status = "thinking…".into();
    term.draw(|f| draw(f, app))?;

    let msgs = app.to_messages();
    let result = p.chat(&msgs, &mut |tok| {
        app.append_assistant(tok);
        let _ = term.draw(|f| draw(f, app));
    });
    app.status = "ready".into();
    if let Err(e) = result {
        app.append_assistant(&format!("\n[error: {e}]"));
    }
    Ok(())
}
