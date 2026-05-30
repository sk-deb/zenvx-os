//! Interactive ZenvX terminal REPL. Reads chat input, streams replies, and runs
//! tool calls through the safety gate (asking twice for risky commands).

use std::io::{self, BufRead, Write};
use zenvx_agent::{Agent, Confirmer, ShellExecutor};
use zenvx_ai_core::from_config;
use zenvx_common::config::Config;
use zenvx_repl::Repl;

struct CliConfirmer;
impl Confirmer for CliConfirmer {
    fn confirm(&mut self, prompt: &str) -> bool {
        eprint!("{prompt} [y/N]: ");
        io::stderr().flush().ok();
        let mut s = String::new();
        io::stdin().lock().read_line(&mut s).ok();
        matches!(s.trim().to_lowercase().as_str(), "y" | "yes")
    }
}

fn main() {
    let Some(cfg) = Config::load() else {
        eprintln!("No config — run `zenvx` first to choose a provider.");
        std::process::exit(1);
    };
    let provider = match from_config(&cfg) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let mut repl = Repl::new(provider, Agent::new(CliConfirmer, ShellExecutor));
    println!("ZenvX REPL [{}] — type 'exit' to quit.", cfg.provider.as_str());

    let stdin = io::stdin();
    loop {
        print!("\nyou> ");
        io::stdout().flush().ok();
        let mut line = String::new();
        if stdin.lock().read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let line = line.trim();
        if line == "exit" || line == "quit" {
            break;
        }
        if line.is_empty() {
            continue;
        }
        print!("ai> ");
        let mut out = io::stdout();
        let res = repl.handle_turn(line, &mut |t| {
            print!("{t}");
            out.flush().ok();
        });
        println!();
        if let Err(e) = res {
            eprintln!("error: {e}");
        }
    }
}
