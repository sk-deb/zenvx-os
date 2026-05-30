//! `ai-core ask "<prompt>"` — streams a reply from the configured provider.

use std::io::Write;
use zenvx_ai_core::{from_config, Message};
use zenvx_common::config::Config;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 || args[1] != "ask" {
        eprintln!("usage: ai-core ask \"<prompt>\"");
        std::process::exit(2);
    }
    let prompt = args[2..].join(" ");

    let Some(cfg) = Config::load() else {
        eprintln!("No config yet — run `zenvx` first to choose a provider.");
        std::process::exit(1);
    };
    let provider = match from_config(&cfg) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    eprintln!("[{}] streaming...", provider.name());
    let mut out = std::io::stdout();
    let res = provider.chat(&[Message::user(prompt)], &mut |t| {
        print!("{t}");
        out.flush().ok();
    });
    println!();
    if let Err(e) = res {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
