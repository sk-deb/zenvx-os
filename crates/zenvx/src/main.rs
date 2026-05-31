//! ZenvX OS control binary.
//! First boot runs the provider setup; later runs report the active config.

use std::io::{self, BufRead, Write};
use zenvx_common::config::{config_path, Config, Provider};
use zenvx_common::{NAME, VERSION};

/// Small default that fits the low-RAM target machine.
const DEFAULT_LOCAL_MODEL: &str = "llama3.2:1b";

fn prompt(msg: &str) -> Option<String> {
    print!("{msg}");
    io::stdout().flush().ok();
    let mut s = String::new();
    match io::stdin().lock().read_line(&mut s) {
        Ok(0) | Err(_) => None, // EOF or error — caller should stop, not loop
        Ok(_) => Some(s.trim().to_string()),
    }
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

/// Interactive first-boot flow. Loops until a provider is chosen (or stdin ends).
fn first_boot_setup() -> Config {
    println!("Welcome to {NAME} — first-boot setup.\n");
    loop {
        match prompt("Enter your OpenRouter API key (leave blank to skip): ") {
            None => break, // stdin closed — stop instead of spinning
            Some(key) if !key.is_empty() => {
                println!("OpenRouter set as the default provider.");
                return Config {
                    provider: Provider::OpenRouter,
                    model: Some("openrouter/auto".into()),
                    openrouter_key: Some(key),
                };
            }
            Some(_) => {}
        }

        match prompt("No key given. Switch to a local Ollama model instead? [y/N]: ")
            .map(|s| s.to_lowercase())
        {
            None => break,
            Some(ans) if ans == "y" || ans == "yes" => {
                let model = detect_ollama_model().unwrap_or_else(|| {
                    println!("No local model installed; defaulting to '{DEFAULT_LOCAL_MODEL}' (run: ollama pull {DEFAULT_LOCAL_MODEL}).");
                    DEFAULT_LOCAL_MODEL.into()
                });
                println!("Using local Ollama model: {model}");
                return Config { provider: Provider::Ollama, model: Some(model), openrouter_key: None };
            }
            _ => {}
        }

        println!("Okay — let's try the API key again.\n");
    }
    // stdin ended without a choice: default to OpenRouter (no key); UI prompts later.
    Config { provider: Provider::OpenRouter, model: Some("openrouter/auto".into()), openrouter_key: None }
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
