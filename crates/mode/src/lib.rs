//! Mode manager: turns hardware + network facts into runtime policy —
//! online-first reasoning, whether a local LLM is viable, and whether the VM
//! path is allowed. Auto-degrades on weak hardware (the 3GB target).

/// Reasoning backend to prefer.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Mode {
    Online,
    Local,
}

/// Detected (or simulated) machine + connectivity facts.
#[derive(Debug, Clone, Copy)]
pub struct Specs {
    pub ram_mb: u64,
    pub cpu_cores: usize,
    pub has_kvm: bool,
    pub has_gpu: bool,
    pub online: bool,
}

/// Policy derived from specs.
#[derive(Debug, PartialEq, Eq)]
pub struct Decision {
    pub mode: Mode,
    pub vm_enabled: bool,
    pub allow_local_llm: bool,
}

/// RAM needed before a local LLM is worth offering.
const LOCAL_LLM_MIN_RAM_MB: u64 = 8192;
/// RAM needed (with KVM) before the VM path is allowed.
const VM_MIN_RAM_MB: u64 = 4096;

/// Decide runtime policy. `override_mode` lets settings force a mode manually.
pub fn decide(specs: &Specs, override_mode: Option<Mode>) -> Decision {
    let allow_local_llm = specs.ram_mb >= LOCAL_LLM_MIN_RAM_MB;
    let vm_enabled = specs.has_kvm && specs.ram_mb >= VM_MIN_RAM_MB;

    // Online-first: prefer the cloud when reachable; otherwise fall back to a
    // local model only if the machine can actually run one.
    let auto = if specs.online {
        Mode::Online
    } else if allow_local_llm {
        Mode::Local
    } else {
        Mode::Online // offline + too weak: stay online-first so it works once reconnected
    };

    Decision { mode: override_mode.unwrap_or(auto), vm_enabled, allow_local_llm }
}

impl Specs {
    pub fn detect() -> Self {
        Self {
            ram_mb: total_ram_mb().unwrap_or(0),
            cpu_cores: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
            has_kvm: std::path::Path::new("/dev/kvm").exists(),
            has_gpu: std::path::Path::new("/dev/dri/renderD128").exists(),
            online: is_online(),
        }
    }
}

fn total_ram_mb() -> Option<u64> {
    let s = std::fs::read_to_string("/proc/meminfo").ok()?;
    let line = s.lines().find(|l| l.starts_with("MemTotal:"))?;
    Some(line.split_whitespace().nth(1)?.parse::<u64>().ok()? / 1024)
}

/// Best-effort connectivity check: can we open a TCP connection out?
fn is_online() -> bool {
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::Duration;
    "1.1.1.1:443"
        .to_socket_addrs()
        .ok()
        .and_then(|mut a| a.next())
        .map(|addr| TcpStream::connect_timeout(&addr, Duration::from_millis(800)).is_ok())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn specs(ram_mb: u64, has_kvm: bool, online: bool) -> Specs {
        Specs { ram_mb, cpu_cores: 4, has_kvm, has_gpu: false, online }
    }

    #[test]
    fn weak_target_is_online_first_vm_off() {
        let d = decide(&specs(3700, true, true), None); // 3GB target, online
        assert_eq!(d.mode, Mode::Online);
        assert!(!d.vm_enabled);
        assert!(!d.allow_local_llm);
    }

    #[test]
    fn strong_machine_unlocks_local_and_vm() {
        let d = decide(&specs(16000, true, true), None);
        assert_eq!(d.mode, Mode::Online); // still online-first by default
        assert!(d.vm_enabled);
        assert!(d.allow_local_llm);
    }

    #[test]
    fn offline_strong_falls_back_to_local() {
        let d = decide(&specs(16000, true, false), None);
        assert_eq!(d.mode, Mode::Local);
    }

    #[test]
    fn manual_override_wins() {
        let d = decide(&specs(3700, true, true), Some(Mode::Local));
        assert_eq!(d.mode, Mode::Local);
    }
}
