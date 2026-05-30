//! Terminal LLM REPL: wires `ai-core` (the brain) and `agent` (safe tools) into a
//! chat loop with a persistent session and inline, safety-gated tool execution.

use zenvx_agent::{Agent, Confirmer, Executor, Tool};
use zenvx_ai_core::{LlmProvider, Message};
use zenvx_common::Result;

const SYSTEM: &str = "You are the ZenvX OS assistant in a terminal with system access.\n\
To use a tool, output one line exactly: `@tool <name> <arg>`\n\
Tools: run_shell <command>, open_app <name>, list_files <path>.\n\
Use at most one tool per reply; afterwards you receive `[tool] <output>`.\n\
If no tool is needed, just answer.";

/// Max tool round-trips per user turn (prevents runaway loops).
const MAX_TOOL_HOPS: usize = 5;

fn parse_tool(reply: &str) -> Option<Tool> {
    for line in reply.lines() {
        if let Some(rest) = line.trim().strip_prefix("@tool ") {
            let mut it = rest.splitn(2, ' ');
            let name = it.next()?.trim();
            let arg = it.next().unwrap_or("").trim().to_string();
            return match name {
                "run_shell" => Some(Tool::RunShell(arg)),
                "open_app" => Some(Tool::OpenApp(arg)),
                "list_files" => Some(Tool::ListFiles(arg)),
                _ => None,
            };
        }
    }
    None
}

pub struct Repl<C: Confirmer, E: Executor> {
    provider: Box<dyn LlmProvider>,
    pub agent: Agent<C, E>,
    pub messages: Vec<Message>,
}

impl<C: Confirmer, E: Executor> Repl<C, E> {
    pub fn new(provider: Box<dyn LlmProvider>, agent: Agent<C, E>) -> Self {
        Self { provider, agent, messages: vec![Message::system(SYSTEM)] }
    }

    /// Handle one user turn: stream the reply, run any tool call (through the
    /// safety gate), feed the result back, and continue until a plain answer.
    pub fn handle_turn(&mut self, input: &str, on_token: &mut dyn FnMut(&str)) -> Result<String> {
        self.messages.push(Message::user(input));
        let mut last = String::new();
        for _ in 0..MAX_TOOL_HOPS {
            let reply = self.provider.chat(&self.messages, on_token)?;
            self.messages.push(Message { role: "assistant".into(), content: reply.clone() });
            last = reply.clone();
            match parse_tool(&reply) {
                Some(tool) => {
                    let result = self.agent.dispatch(tool).unwrap_or_else(|e| format!("error: {e}"));
                    let fed = format!("[tool] {result}");
                    on_token(&format!("\n{fed}\n"));
                    self.messages.push(Message::user(fed));
                }
                None => return Ok(reply),
            }
        }
        Ok(last)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct Mock {
        replies: Vec<String>,
        idx: Cell<usize>,
    }
    impl LlmProvider for Mock {
        fn name(&self) -> &'static str {
            "mock"
        }
        fn chat(&self, _m: &[Message], on: &mut dyn FnMut(&str)) -> Result<String> {
            let i = self.idx.get();
            self.idx.set(i + 1);
            let r = self.replies.get(i).cloned().unwrap_or_default();
            on(&r);
            Ok(r)
        }
    }
    fn mock(replies: &[&str]) -> Box<Mock> {
        Box::new(Mock { replies: replies.iter().map(|s| s.to_string()).collect(), idx: Cell::new(0) })
    }

    struct Scripted {
        answers: Vec<bool>,
        asked: usize,
    }
    impl Confirmer for Scripted {
        fn confirm(&mut self, _p: &str) -> bool {
            let a = self.answers.get(self.asked).copied().unwrap_or(false);
            self.asked += 1;
            a
        }
    }
    #[derive(Default)]
    struct Recording {
        calls: Vec<String>,
    }
    impl Executor for Recording {
        fn run(&mut self, c: &str) -> Result<String> {
            self.calls.push(c.into());
            Ok("ran".into())
        }
    }

    #[test]
    fn runs_safe_tool_then_answers() {
        let agent = Agent::new(Scripted { answers: vec![], asked: 0 }, Recording::default());
        let mut repl = Repl::new(mock(&["@tool run_shell echo hello", "All done!"]), agent);
        let mut out = String::new();
        let final_reply = repl.handle_turn("greet", &mut |t| out.push_str(t)).unwrap();
        assert_eq!(final_reply, "All done!");
        assert_eq!(repl.agent.executor.calls, vec!["echo hello"]);
        assert!(out.contains("[tool] ran"));
        assert_eq!(repl.agent.confirmer.asked, 0); // safe -> no confirmation
    }

    #[test]
    fn risky_tool_gated_in_repl() {
        let agent = Agent::new(Scripted { answers: vec![true, true], asked: 0 }, Recording::default());
        let mut repl = Repl::new(mock(&["@tool run_shell sudo reboot", "Rebooting."]), agent);
        repl.handle_turn("restart", &mut |_| {}).unwrap();
        assert_eq!(repl.agent.executor.calls, vec!["sudo reboot"]);
        assert_eq!(repl.agent.confirmer.asked, 2); // double-confirmed before running
    }

    #[test]
    fn denied_tool_blocked_in_repl() {
        let agent = Agent::new(Scripted { answers: vec![true, true], asked: 0 }, Recording::default());
        let mut repl = Repl::new(mock(&["@tool run_shell rm -rf /", "ok"]), agent);
        let mut out = String::new();
        repl.handle_turn("wipe", &mut |t| out.push_str(t)).unwrap();
        assert!(repl.agent.executor.calls.is_empty()); // never executed
        assert!(out.contains("[tool] error")); // blocked message fed back to the model
    }
}
