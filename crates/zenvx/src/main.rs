//! ZenvX OS control binary.
//! First boot runs the provider setup; later runs report the active config.

use std::io::{self, BufRead, Write};
use zenvx_common::config::{config_path, Config, Provider};
use zenvx_common::{NAME, VERSION};

/// Small default that fits the low-RAM target machine.
const DEFAULT_LOCAL_MODEL: &str = "llama3.2:1b";

fn prompt(msg: &str) -> String {
    print!("{msg}");
    io::stdout().flush().ok();
    let mut s = String::new();
    io::stdin().lock().read_line(&mut s).ok();
    s.trim().to_string()
}

/// First installed Ollama model, if any (`ollama list`, skip header, take col 0).
fn detect_ollama_model() -> Option<String> {
    let out = std::process::Command::new("ollama").arg("list").output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .skip(1)
        .filter_map(|l| l.split_whitespace().next())
        .next()
        .map(str::to_string)
}

/// Interactive first-boot flow. Loops until a provider is chosen.
fn first_boot_setup() -> Config {
    println!("Welcome to {NAME} — first-boot setup.\n");
    loop {
        let key = prompt("Enter your OpenRouter API key (leave blank to skip): ");
        if !key.is_empty() {
            println!("OpenRouter set as the default provider.");
            return Config {
                provider: Provider::OpenRouter,
                model: Some("openrouter/auto".into()),
                openrouter_key: Some(key),
            };
        }

        let ans = prompt("No key given. Switch to a local Ollama model instead? [y/N]: ")
            .to_lowercase();
        if ans == "y" || ans == "yes" {
            let model = detect_ollama_model().unwrap_or_else(|| {
                println!("No local model installed; defaulting to '{DEFAULT_LOCAL_MODEL}' (run: ollama pull {DEFAULT_LOCAL_MODEL}).");
                DEFAULT_LOCAL_MODEL.into()
            });
            println!("Using local Ollama model: {model}");
            return Config { provider: Provider::Ollama, model: Some(model), openrouter_key: None };
        }

        println!("Okay — let's try the API key again.\n");
    }
}

fn main() {
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("{NAME} {VERSION}");
        return;
    }

    match Config::load() {
        Some(cfg) => {
            print!("{NAME} {VERSION} ready. Provider: {}", cfg.provider.as_str());
            if let Some(m) = &cfg.model {
                print!("  Model: {m}");
            }
            println!();
        }
        None => {
            let cfg = first_boot_setup();
            match cfg.save() {
                Ok(()) => println!("Saved config to {}", config_path().display()),
                Err(e) => eprintln!("Failed to save config: {e}"),
            }
        }
    }
}
