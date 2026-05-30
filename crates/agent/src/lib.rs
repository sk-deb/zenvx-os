//! Agent tool layer with a safety gate.
//!
//! Tools: `run_shell` (gated), `open_app`, `list_files`.
//! Catastrophic commands are hard-denied (never run). Root/destructive commands
//! require *two* confirmations before execution.

use zenvx_common::{Error, Result};

/// Risk level assigned to a shell command.
#[derive(Debug, PartialEq, Eq)]
pub enum Risk {
    Safe,
    Confirm,
    Denied,
}

/// Catastrophic substrings — never executed, even if the user confirms.
const DENY: &[&str] = &[
    ":(){",            // fork bomb
    "mkfs",            // format a filesystem
    "wipefs",          // wipe filesystem signatures
    "of=/dev/sd",      // dd onto a raw disk
    "of=/dev/nvme",
    "of=/dev/vd",
    "> /dev/sd",
    "--no-preserve-root",
];

/// Root/destructive markers — require double confirmation.
const CONFIRM: &[&str] = &[
    "sudo", "rm ", "rmdir", "dd ", "shutdown", "reboot", "poweroff", "mount", "umount",
    "pacman", "systemctl", "chown", "chmod", "kill", "mkswap", "fdisk", "parted",
];

/// Top-level targets that make a recursive `rm` catastrophic.
fn rm_target(lc: &str) -> Option<&str> {
    for t in ["rm -rf ", "rm -fr ", "rm -r -f ", "rm -f -r "] {
        if let Some(i) = lc.find(t) {
            return Some(lc[i + t.len()..].split_whitespace().next().unwrap_or(""));
        }
    }
    None
}

fn is_catastrophic_rm(lc: &str) -> bool {
    match rm_target(lc) {
        Some(t) => {
            t == "/"
                || t == "/*"
                || matches!(
                    t,
                    "/etc" | "/usr" | "/bin" | "/sbin" | "/boot" | "/lib" | "/lib64" | "/var"
                        | "/home" | "/root" | "/sys" | "/dev" | "/proc" | "/run"
                )
        }
        None => false,
    }
}

/// Classify a shell command's risk.
pub fn classify(cmd: &str) -> Risk {
    let lc = cmd.to_lowercase();
    if is_catastrophic_rm(&lc) || DENY.iter().any(|p| lc.contains(p)) {
        Risk::Denied
    } else if CONFIRM.iter().any(|p| lc.contains(p)) {
        Risk::Confirm
    } else {
        Risk::Safe
    }
}

/// Asks the user to approve a risky action (called twice for double-confirm).
pub trait Confirmer {
    fn confirm(&mut self, prompt: &str) -> bool;
}

/// Runs a shell command and returns its combined stdout+stderr.
pub trait Executor {
    fn run(&mut self, command: &str) -> Result<String>;
}

/// Real executor: runs commands via `sh -c`.
pub struct ShellExecutor;
impl Executor for ShellExecutor {
    fn run(&mut self, command: &str) -> Result<String> {
        let out = std::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .map_err(|e| Error::Msg(e.to_string()))?;
        let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
        s.push_str(&String::from_utf8_lossy(&out.stderr));
        Ok(s)
    }
}

/// Tools the agent can invoke on the system.
pub enum Tool {
    RunShell(String),
    OpenApp(String),
    ListFiles(String),
}

pub struct Agent<C: Confirmer, E: Executor> {
    pub confirmer: C,
    pub executor: E,
}

impl<C: Confirmer, E: Executor> Agent<C, E> {
    pub fn new(confirmer: C, executor: E) -> Self {
        Self { confirmer, executor }
    }

    pub fn dispatch(&mut self, tool: Tool) -> Result<String> {
        match tool {
            Tool::RunShell(cmd) => self.run_shell(&cmd),
            Tool::OpenApp(name) => self.open_app(&name),
            Tool::ListFiles(path) => list_files(&path),
        }
    }

    fn run_shell(&mut self, cmd: &str) -> Result<String> {
        match classify(cmd) {
            Risk::Denied => {
                Err(Error::Msg(format!("blocked: '{cmd}' is on the denylist and will not run")))
            }
            Risk::Safe => self.executor.run(cmd),
            Risk::Confirm => {
                let p1 =
                    format!("This may need root or is destructive:\n  {cmd}\nConfirm? (1/2)");
                if !self.confirmer.confirm(&p1) {
                    return Err(Error::Msg("cancelled by user".into()));
                }
                if !self.confirmer.confirm("Are you absolutely sure? (2/2)") {
                    return Err(Error::Msg("cancelled by user".into()));
                }
                self.executor.run(cmd)
            }
        }
    }

    fn open_app(&mut self, name: &str) -> Result<String> {
        // Route through the launcher (.exe->Wine, AppImage, flatpak id, pacman,
        // VM) — it quotes args so the name can't break out of the shell.
        let cmd = zenvx_launcher::resolve(name)?;
        self.executor.run(&format!("setsid -f {cmd} >/dev/null 2>&1"))
    }
}

fn list_files(path: &str) -> Result<String> {
    let dir = if path.is_empty() { "." } else { path };
    let mut out = String::new();
    for entry in std::fs::read_dir(dir).map_err(|e| Error::Msg(e.to_string()))? {
        let entry = entry.map_err(|e| Error::Msg(e.to_string()))?;
        out.push_str(&entry.file_name().to_string_lossy());
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scripted {
        answers: Vec<bool>,
        asked: usize,
    }
    impl Scripted {
        fn new(a: Vec<bool>) -> Self {
            Self { answers: a, asked: 0 }
        }
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
            self.calls.push(c.to_string());
            Ok("ran".into())
        }
    }

    #[test]
    fn classify_levels() {
        assert_eq!(classify("ls -la"), Risk::Safe);
        assert_eq!(classify("sudo pacman -Syu"), Risk::Confirm);
        assert_eq!(classify("rm -rf /"), Risk::Denied);
        assert_eq!(classify("rm -rf /home"), Risk::Denied);
        assert_eq!(classify(":(){ :|:& };:"), Risk::Denied);
        // a deep path is destructive but not catastrophic -> confirm, not deny
        assert_eq!(classify("rm -rf /home/rye/project/target"), Risk::Confirm);
    }

    #[test]
    fn denied_never_executes() {
        let mut a = Agent::new(Scripted::new(vec![true, true]), Recording::default());
        assert!(a.dispatch(Tool::RunShell("rm -rf /".into())).is_err());
        assert!(a.executor.calls.is_empty()); // executor never touched
    }

    #[test]
    fn safe_runs_without_confirm() {
        let mut a = Agent::new(Scripted::new(vec![]), Recording::default());
        a.dispatch(Tool::RunShell("echo hi".into())).unwrap();
        assert_eq!(a.executor.calls, vec!["echo hi"]);
        assert_eq!(a.confirmer.asked, 0);
    }

    #[test]
    fn risky_needs_two_confirms() {
        // both yes -> runs, asked exactly twice
        let mut a = Agent::new(Scripted::new(vec![true, true]), Recording::default());
        a.dispatch(Tool::RunShell("sudo reboot".into())).unwrap();
        assert_eq!(a.executor.calls.len(), 1);
        assert_eq!(a.confirmer.asked, 2);

        // first no -> stops after one ask, never runs
        let mut a = Agent::new(Scripted::new(vec![false, true]), Recording::default());
        assert!(a.dispatch(Tool::RunShell("sudo systemctl restart sshd".into())).is_err());
        assert!(a.executor.calls.is_empty());
        assert_eq!(a.confirmer.asked, 1);

        // yes then no -> asked twice, never runs
        let mut a = Agent::new(Scripted::new(vec![true, false]), Recording::default());
        assert!(a.dispatch(Tool::RunShell("sudo pacman -Rns foo".into())).is_err());
        assert!(a.executor.calls.is_empty());
        assert_eq!(a.confirmer.asked, 2);
    }

    #[test]
    fn open_app_routes_and_quotes() {
        let mut a = Agent::new(Scripted::new(vec![]), Recording::default());
        // a malicious name is single-quoted by the launcher, so it can't break out
        a.dispatch(Tool::OpenApp("firefox; rm -rf /".into())).unwrap();
        assert!(a.executor.calls[0].contains("setsid -f 'firefox; rm -rf /'"));
        // a Windows exe routes through Wine
        a.dispatch(Tool::OpenApp("game.exe".into())).unwrap();
        assert!(a.executor.calls[1].contains("wine 'game.exe'"));
    }

    #[test]
    fn list_files_reads_dir() {
        assert!(list_files(".").unwrap().contains("Cargo.toml"));
    }
}
