//! Voice pipeline: wake word -> speech-to-text -> agent -> text-to-speech.
//!
//! The stages are traits so the flow is testable with mocks. Real backends
//! shell out to whisper.cpp (STT) and Piper (TTS) when installed; if they are
//! missing the pipeline degrades gracefully (callers fall back to typed input).

use std::process::Command;
use zenvx_common::{Error, Result};

pub trait WakeWord {
    /// Block until the wake word is heard; returns false if listening stopped.
    fn wait_for_wake(&mut self) -> bool;
}

pub trait Stt {
    /// Transcribe an audio file to text.
    fn transcribe(&mut self, audio_path: &str) -> Result<String>;
}

pub trait Tts {
    /// Speak the given text aloud.
    fn speak(&mut self, text: &str) -> Result<()>;
}

fn shq(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn have(bin: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {}", shq(bin)))
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Real STT via whisper.cpp (`whisper-cli -m model -f audio -nt -otxt`).
pub struct WhisperCli {
    pub binary: String,
    pub model: String,
}
impl Stt for WhisperCli {
    fn transcribe(&mut self, audio_path: &str) -> Result<String> {
        let out = Command::new(&self.binary)
            .args(["-m", &self.model, "-f", audio_path, "-nt", "-otxt"])
            .output()
            .map_err(|e| Error::Msg(e.to_string()))?;
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }
}

/// Real TTS via Piper, piped to an audio player.
pub struct PiperTts {
    pub binary: String,
    pub model: String,
    pub player: String, // e.g. "aplay" or "paplay"
}
impl Tts for PiperTts {
    fn speak(&mut self, text: &str) -> Result<()> {
        let wav = "/tmp/zenvx_tts.wav";
        let cmd = format!(
            "echo {} | {} -m {} --output_file {wav} && {} {wav}",
            shq(text),
            shq(&self.binary),
            shq(&self.model),
            shq(&self.player),
        );
        let status = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .status()
            .map_err(|e| Error::Msg(e.to_string()))?;
        if status.success() {
            Ok(())
        } else {
            Err(Error::Msg("TTS playback failed".into()))
        }
    }
}

pub struct VoicePipeline<W: WakeWord, S: Stt, T: Tts> {
    pub wake: W,
    pub stt: S,
    pub tts: T,
}

impl<W: WakeWord, S: Stt, T: Tts> VoicePipeline<W, S, T> {
    pub fn new(wake: W, stt: S, tts: T) -> Self {
        Self { wake, stt, tts }
    }

    /// One interaction: wait for the wake word, transcribe `audio_path`, get a
    /// reply from `respond` (the agent/REPL), speak it. `None` if not woken.
    pub fn once(
        &mut self,
        audio_path: &str,
        respond: &mut dyn FnMut(&str) -> Result<String>,
    ) -> Result<Option<(String, String)>> {
        if !self.wake.wait_for_wake() {
            return Ok(None);
        }
        let transcript = self.stt.transcribe(audio_path)?;
        let response = respond(&transcript)?;
        self.tts.speak(&response)?;
        Ok(Some((transcript, response)))
    }
}

/// Whether local voice backends are installed (for graceful degradation).
pub struct VoiceAvailability {
    pub whisper: bool,
    pub piper: bool,
}
impl VoiceAvailability {
    pub fn detect() -> Self {
        Self { whisper: have("whisper-cli") || have("whisper"), piper: have("piper") }
    }
    pub fn ready(&self) -> bool {
        self.whisper && self.piper
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct MockWake(bool);
    impl WakeWord for MockWake {
        fn wait_for_wake(&mut self) -> bool {
            self.0
        }
    }
    struct MockStt(String);
    impl Stt for MockStt {
        fn transcribe(&mut self, _a: &str) -> Result<String> {
            Ok(self.0.clone())
        }
    }
    struct MockTts<'a>(&'a RefCell<Vec<String>>);
    impl Tts for MockTts<'_> {
        fn speak(&mut self, text: &str) -> Result<()> {
            self.0.borrow_mut().push(text.into());
            Ok(())
        }
    }

    #[test]
    fn wake_gate_blocks_when_not_woken() {
        let spoken = RefCell::new(vec![]);
        let mut p = VoicePipeline::new(MockWake(false), MockStt("hi".into()), MockTts(&spoken));
        let r = p.once("x.wav", &mut |_| Ok("ignored".into())).unwrap();
        assert!(r.is_none());
        assert!(spoken.borrow().is_empty());
    }

    #[test]
    fn full_flow_transcribes_responds_and_speaks() {
        let spoken = RefCell::new(vec![]);
        let mut p =
            VoicePipeline::new(MockWake(true), MockStt("open firefox".into()), MockTts(&spoken));
        let r = p
            .once("rec.wav", &mut |t| {
                assert_eq!(t, "open firefox");
                Ok(format!("Opening {}", t.trim_start_matches("open ")))
            })
            .unwrap();
        assert_eq!(r, Some(("open firefox".into(), "Opening firefox".into())));
        assert_eq!(*spoken.borrow(), vec!["Opening firefox"]);
    }
}
