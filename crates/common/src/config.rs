//! ZenvX configuration: which AI provider is the default, and its model/key.
//! Stored at `$XDG_CONFIG_HOME/zenvx/config` (0600, never in the repo).

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provider {
    OpenRouter,
    Ollama,
}

impl Provider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Provider::OpenRouter => "openrouter",
            Provider::Ollama => "ollama",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "openrouter" => Some(Provider::OpenRouter),
            "ollama" => Some(Provider::Ollama),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub provider: Provider,
    pub model: Option<String>,
    pub openrouter_key: Option<String>,
}

pub fn config_dir() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        format!("{home}/.config")
    });
    PathBuf::from(base).join("zenvx")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config")
}

impl Config {
    /// Load existing config, or `None` on first boot (no file yet).
    pub fn load() -> Option<Config> {
        let txt = std::fs::read_to_string(config_path()).ok()?;
        let (mut provider, mut model, mut key) = (None, None, None);
        for line in txt.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                let v = v.trim().to_string();
                match k.trim() {
                    "provider" => provider = Provider::parse(&v),
                    "model" if !v.is_empty() => model = Some(v),
                    "openrouter_key" if !v.is_empty() => key = Some(v),
                    _ => {}
                }
            }
        }
        Some(Config { provider: provider?, model, openrouter_key: key })
    }

    /// Persist config with 0600 permissions (it may hold the API key).
    pub fn save(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(config_dir())?;
        let mut s = format!("provider={}\n", self.provider.as_str());
        if let Some(m) = &self.model {
            s.push_str(&format!("model={m}\n"));
        }
        if let Some(k) = &self.openrouter_key {
            s.push_str(&format!("openrouter_key={k}\n"));
        }
        let path = config_path();
        std::fs::write(&path, s)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_roundtrip() {
        assert_eq!(Provider::parse("openrouter"), Some(Provider::OpenRouter));
        assert_eq!(Provider::parse("ollama"), Some(Provider::Ollama));
        assert_eq!(Provider::parse("nope"), None);
        assert_eq!(Provider::OpenRouter.as_str(), "openrouter");
    }
}
