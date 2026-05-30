//! Session orchestrator: ties the voice service, agent (+launcher) and UI status
//! together inside the compositor. A voice command like "open firefox" launches
//! the app (into the 80% zone) and updates the UI status line.

use zenvx_agent::{Agent, Confirmer, Executor, Tool};
use zenvx_common::Result;
use zenvx_compositor::{layout, tile, Rect};

/// What the shell UI overlay shows.
#[derive(Debug, Default)]
pub struct UiStatus {
    pub voice: String,
    pub last_launched: Option<String>,
    pub app_surfaces: usize,
}

pub struct Session<C: Confirmer, E: Executor> {
    pub agent: Agent<C, E>,
    pub ui: UiStatus,
    pub screen: (i32, i32),
}

impl<C: Confirmer, E: Executor> Session<C, E> {
    pub fn new(agent: Agent<C, E>, screen: (i32, i32)) -> Self {
        Self { agent, ui: UiStatus::default(), screen }
    }

    /// Handle a transcribed voice command. "open <app>" launches it and bumps
    /// the app-zone surface count; anything else is run as a shell intent.
    pub fn handle_voice(&mut self, transcript: &str) -> Result<String> {
        self.ui.voice = format!("heard: {transcript}");
        if let Some(app) = transcript.strip_prefix("open ") {
            let app = app.trim().to_string();
            self.agent.dispatch(Tool::OpenApp(app.clone()))?;
            self.ui.last_launched = Some(app.clone());
            self.ui.app_surfaces += 1;
            Ok(format!("Opening {app}"))
        } else {
            self.agent.dispatch(Tool::RunShell(transcript.into()))
        }
    }

    /// Current tiling of launched app surfaces within the 80% app zone.
    pub fn app_layout(&self) -> Vec<Rect> {
        let l = layout(self.screen.0, self.screen.1);
        tile(&l.app_zone, self.ui.app_surfaces)
    }
}

/// Spawns a terminal surface bound to a compositor zone.
pub trait PaneSpawner {
    /// Spawn `command` inside `zone`; returns a handle/pid.
    fn spawn_in_zone(&mut self, zone: Rect, command: &str) -> Result<u32>;
}

/// Embed the Task-4 REPL in the reserved 20% terminal zone.
pub fn embed_repl(screen: (i32, i32), spawner: &mut dyn PaneSpawner) -> Result<(Rect, u32)> {
    let zone = layout(screen.0, screen.1).term_zone;
    let pid = spawner.spawn_in_zone(zone, "zenvx-repl")?;
    Ok((zone, pid))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use zenvx_voice::{Stt, Tts, VoicePipeline, WakeWord};

    struct NoConfirm;
    impl Confirmer for NoConfirm {
        fn confirm(&mut self, _p: &str) -> bool {
            true
        }
    }
    #[derive(Default)]
    struct Recording {
        calls: Vec<String>,
    }
    impl Executor for Recording {
        fn run(&mut self, c: &str) -> Result<String> {
            self.calls.push(c.into());
            Ok("ok".into())
        }
    }
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
        fn speak(&mut self, t: &str) -> Result<()> {
            self.0.borrow_mut().push(t.into());
            Ok(())
        }
    }

    #[test]
    fn voice_open_app_flows_through_launch_ui_and_layout() {
        let spoken = RefCell::new(vec![]);
        let mut session = Session::new(Agent::new(NoConfirm, Recording::default()), (1920, 1080));
        let mut pipe = VoicePipeline::new(MockWake(true), MockStt("open firefox".into()), MockTts(&spoken));

        // voice -> STT -> session(agent+launcher+UI) -> TTS
        let result = pipe
            .once("rec.wav", &mut |t| session.handle_voice(t))
            .unwrap();

        assert_eq!(result, Some(("open firefox".into(), "Opening firefox".into())));
        // launcher routed firefox through the agent's executor
        assert!(session.agent.executor.calls[0].contains("'firefox'"));
        // UI status updated
        assert_eq!(session.ui.last_launched.as_deref(), Some("firefox"));
        assert_eq!(session.ui.voice, "heard: open firefox");
        // the app got a surface placed in the 80% app zone
        let rects = session.app_layout();
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0], Rect { x: 0, y: 0, w: 1920, h: 864 });
        // and it was spoken back
        assert_eq!(*spoken.borrow(), vec!["Opening firefox"]);
    }

    struct RecSpawner {
        spawned: Vec<(Rect, String)>,
    }
    impl PaneSpawner for RecSpawner {
        fn spawn_in_zone(&mut self, zone: Rect, command: &str) -> Result<u32> {
            self.spawned.push((zone, command.into()));
            Ok(4242)
        }
    }

    #[test]
    fn repl_embedded_in_terminal_zone() {
        let mut sp = RecSpawner { spawned: vec![] };
        let (zone, pid) = embed_repl((1920, 1080), &mut sp).unwrap();
        assert_eq!(pid, 4242);
        assert_eq!(zone, Rect { x: 0, y: 864, w: 1920, h: 216 }); // the reserved 20% zone
        assert_eq!(sp.spawned[0].1, "zenvx-repl"); // REPL is the hosted process
        assert_eq!(sp.spawned[0].0, zone); // bound to the terminal zone
    }
}
