//! Unified AI adapter: one streaming trait, OpenRouter + Ollama backends,
//! with the active backend selected from `zenvx_common::config`.

use std::io::{BufRead, BufReader};
use zenvx_common::config::{Config, Provider};
use zenvx_common::{Error, Result};

pub const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1";
pub const OLLAMA_URL: &str = "http://localhost:11434";

fn err(e: impl std::fmt::Display) -> Error {
    Error::Msg(e.to_string())
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    pub fn user(c: impl Into<String>) -> Self {
        Self { role: "user".into(), content: c.into() }
    }
    pub fn system(c: impl Into<String>) -> Self {
        Self { role: "system".into(), content: c.into() }
    }
}

fn messages_json(messages: &[Message]) -> Vec<serde_json::Value> {
    messages
        .iter()
        .map(|m| serde_json::json!({"role": m.role, "content": m.content}))
        .collect()
}

/// A chat backend that streams tokens as they arrive.
pub trait LlmProvider {
    /// Stream a completion: `on_token` is called per content chunk; returns the full text.
    fn chat(&self, messages: &[Message], on_token: &mut dyn FnMut(&str)) -> Result<String>;
    fn name(&self) -> &'static str;
}

/// OpenRouter (OpenAI-compatible, Server-Sent-Events stream).
pub struct OpenRouter {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

impl LlmProvider for OpenRouter {
    fn name(&self) -> &'static str {
        "openrouter"
    }
    fn chat(&self, messages: &[Message], on_token: &mut dyn FnMut(&str)) -> Result<String> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": messages_json(messages),
            "stream": true,
        });
        let resp = ureq::post(&format!("{}/chat/completions", self.base_url))
            .set("Authorization", &format!("Bearer {}", self.api_key))
            .set("Content-Type", "application/json")
            .send_string(&body.to_string())
            .map_err(err)?;

        let mut full = String::new();
        for line in BufReader::new(resp.into_reader()).lines() {
            let line = line.map_err(err)?;
            let data = match line.strip_prefix("data:") {
                Some(d) => d.trim(),
                None => continue,
            };
            if data == "[DONE]" {
                break;
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                if let Some(t) = v["choices"][0]["delta"]["content"].as_str() {
                    on_token(t);
                    full.push_str(t);
                }
            }
        }
        Ok(full)
    }
}

/// Local Ollama (newline-delimited JSON stream).
pub struct Ollama {
    pub base_url: String,
    pub model: String,
}

impl LlmProvider for Ollama {
    fn name(&self) -> &'static str {
        "ollama"
    }
    fn chat(&self, messages: &[Message], on_token: &mut dyn FnMut(&str)) -> Result<String> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": messages_json(messages),
            "stream": true,
        });
        let resp = ureq::post(&format!("{}/api/chat", self.base_url))
            .set("Content-Type", "application/json")
            .send_string(&body.to_string())
            .map_err(err)?;

        let mut full = String::new();
        for line in BufReader::new(resp.into_reader()).lines() {
            let line = line.map_err(err)?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                if let Some(t) = v["message"]["content"].as_str() {
                    on_token(t);
                    full.push_str(t);
                }
                if v["done"].as_bool() == Some(true) {
                    break;
                }
            }
        }
        Ok(full)
    }
}

/// Build the active provider from saved config (OpenRouter primary, Ollama fallback).
pub fn from_config(cfg: &Config) -> Result<Box<dyn LlmProvider>> {
    match cfg.provider {
        Provider::OpenRouter => {
            let api_key = cfg
                .openrouter_key
                .clone()
                .ok_or_else(|| Error::Msg("OpenRouter selected but no API key configured".into()))?;
            let model = cfg.model.clone().unwrap_or_else(|| "openrouter/auto".into());
            let base_url =
                std::env::var("ZENVX_OPENROUTER_URL").unwrap_or_else(|_| OPENROUTER_URL.into());
            Ok(Box::new(OpenRouter { base_url, api_key, model }))
        }
        Provider::Ollama => {
            let model = cfg
                .model
                .clone()
                .ok_or_else(|| Error::Msg("Ollama selected but no model configured".into()))?;
            let base_url = std::env::var("ZENVX_OLLAMA_URL").unwrap_or_else(|_| OLLAMA_URL.into());
            Ok(Box::new(Ollama { base_url, model }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    /// One-shot HTTP server returning a fixed raw response, for stream testing.
    fn mock_server(response: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            if let Ok((mut sock, _)) = listener.accept() {
                let mut buf = [0u8; 4096];
                let _ = sock.read(&mut buf);
                let _ = sock.write_all(response.as_bytes());
            }
        });
        format!("http://{addr}")
    }

    #[test]
    fn openrouter_streams_and_assembles() {
        let resp = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
data: {\"choices\":[{\"delta\":{\"content\":\"Hel\"}}]}\n\
data: {\"choices\":[{\"delta\":{\"content\":\"lo\"}}]}\n\
data: [DONE]\n";
        let p = OpenRouter { base_url: mock_server(resp), api_key: "k".into(), model: "m".into() };
        let mut chunks = Vec::new();
        let full = p.chat(&[Message::user("hi")], &mut |t| chunks.push(t.to_string())).unwrap();
        assert_eq!(full, "Hello");
        assert_eq!(chunks, vec!["Hel", "lo"]);
    }

    #[test]
    fn ollama_streams_and_assembles() {
        let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nConnection: close\r\n\r\n\
{\"message\":{\"content\":\"Hel\"},\"done\":false}\n\
{\"message\":{\"content\":\"lo\"},\"done\":false}\n\
{\"done\":true}\n";
        let p = Ollama { base_url: mock_server(resp), model: "m".into() };
        let mut chunks = Vec::new();
        let full = p.chat(&[Message::user("hi")], &mut |t| chunks.push(t.to_string())).unwrap();
        assert_eq!(full, "Hello");
        assert_eq!(chunks, vec!["Hel", "lo"]);
    }
}
