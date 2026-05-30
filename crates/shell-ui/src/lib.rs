//! Shell UI settings state — the model the settings panel binds to.
//!
//! This is the testable backend of the overlay (launcher/voice-status/settings).
//! The visual layer (native layer-shell overlay, or Tauri) renders these values;
//! the persistence + key-masking logic lives here and reuses the secure config.

use zenvx_common::config::{Config, Provider};
use zenvx_common::{Error, Result};

pub struct Settings {
    cfg: Config,
}

impl Settings {
    /// Load saved settings, or a sensible OpenRouter-primary default.
    pub fn load_or_default() -> Self {
        let cfg = Config::load().unwrap_or(Config {
            provider: Provider::OpenRouter,
            model: Some("openrouter/auto".into()),
            openrouter_key: None,
        });
        Self { cfg }
    }

    pub fn provider(&self) -> &'static str {
        self.cfg.provider.as_str()
    }
    pub fn model(&self) -> Option<&str> {
        self.cfg.model.as_deref()
    }

    /// Key masked for display — the raw key is never surfaced to the UI.
    pub fn masked_key(&self) -> String {
        match &self.cfg.openrouter_key {
            Some(k) if k.len() >= 4 => format!("****{}", &k[k.len() - 4..]),
            Some(_) => "****".into(),
            None => "(none)".into(),
        }
    }

    /// Settings panel: save an OpenRouter key and make it the active provider.
    pub fn set_openrouter(&mut self, key: &str, model: Option<&str>) -> Result<()> {
        self.cfg.provider = Provider::OpenRouter;
        self.cfg.openrouter_key = Some(key.to_string());
        if let Some(m) = model {
            self.cfg.model = Some(m.into());
        } else if self.cfg.model.is_none() {
            self.cfg.model = Some("openrouter/auto".into());
        }
        self.cfg.save().map_err(|e| Error::Msg(e.to_string()))
    }

    /// Settings panel: switch to a local Ollama model.
    pub fn set_local(&mut self, model: &str) -> Result<()> {
        self.cfg.provider = Provider::Ollama;
        self.cfg.model = Some(model.into());
        self.cfg.save().map_err(|e| Error::Msg(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_persist_and_key_is_masked() {
        let tmp = std::env::temp_dir().join(format!("zenvx-ui-{}", std::process::id()));
        std::env::set_var("XDG_CONFIG_HOME", &tmp);

        let mut s = Settings::load_or_default();
        s.set_openrouter("sk-or-abcd1234", Some("openrouter/auto")).unwrap();

        // reload from disk -> values persisted
        let s2 = Settings::load_or_default();
        assert_eq!(s2.provider(), "openrouter");
        assert_eq!(s2.model(), Some("openrouter/auto"));
        assert_eq!(s2.masked_key(), "****1234"); // never shows the full key

        // switch to a local model, reload
        let mut s3 = Settings::load_or_default();
        s3.set_local("llama3.2:1b").unwrap();
        let s4 = Settings::load_or_default();
        assert_eq!(s4.provider(), "ollama");
        assert_eq!(s4.model(), Some("llama3.2:1b"));

        std::fs::remove_dir_all(&tmp).ok();
    }
}
