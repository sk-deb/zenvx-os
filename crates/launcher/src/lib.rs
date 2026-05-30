//! App launcher / compatibility router.
//!
//! Detects what a target is and how to run it: Windows `.exe` via Wine,
//! Linux `.AppImage` directly, Flatpak by app-id, otherwise a pacman-installed
//! command. Disk images route to a QEMU/KVM VM — but only if the hardware can
//! handle it (auto-disabled on the low-RAM target machine).

use zenvx_common::{Error, Result};

#[derive(Debug, PartialEq, Eq)]
pub enum Target {
    WindowsExe(String),
    AppImage(String),
    Flatpak(String),
    Pacman(String),
    Vm(String),
}

/// Reverse-DNS-style Flatpak app id, e.g. `org.mozilla.firefox`.
fn is_flatpak_id(s: &str) -> bool {
    !s.contains('/') && !s.contains(' ') && s.split('.').count() >= 3
}

pub fn classify_target(spec: &str) -> Target {
    let lc = spec.to_lowercase();
    let ends = |e: &str| lc.ends_with(e);
    if ends(".exe") || ends(".msi") {
        Target::WindowsExe(spec.into())
    } else if ends(".appimage") {
        Target::AppImage(spec.into())
    } else if ends(".iso") || ends(".img") || ends(".qcow2") || ends(".vhd") {
        Target::Vm(spec.into())
    } else if is_flatpak_id(spec) {
        Target::Flatpak(spec.into())
    } else {
        Target::Pacman(spec.into())
    }
}

/// Hardware capabilities relevant to launching.
pub struct Capabilities {
    pub has_kvm: bool,
    pub ram_mb: u64,
}

impl Capabilities {
    pub fn detect() -> Self {
        let has_kvm = std::path::Path::new("/dev/kvm").exists();
        Self { has_kvm, ram_mb: total_ram_mb().unwrap_or(0) }
    }
    /// A usable VM needs KVM and enough RAM (the 3GB target falls short on purpose).
    pub fn vm_capable(&self) -> bool {
        self.has_kvm && self.ram_mb >= 4096
    }
}

fn total_ram_mb() -> Option<u64> {
    let s = std::fs::read_to_string("/proc/meminfo").ok()?;
    let line = s.lines().find(|l| l.starts_with("MemTotal:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024)
}

/// Single-quote a value so it cannot break out of the shell command.
fn shq(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Build the shell command to launch a target, honoring hardware limits.
pub fn launch_command(target: &Target, caps: &Capabilities) -> Result<String> {
    Ok(match target {
        Target::WindowsExe(p) => format!("wine {}", shq(p)),
        Target::AppImage(p) => format!("chmod +x {q} && {q}", q = shq(p)),
        Target::Flatpak(id) => format!("flatpak run {}", shq(id)),
        Target::Pacman(name) => shq(name),
        Target::Vm(img) => {
            if !caps.vm_capable() {
                return Err(Error::Msg(format!(
                    "VM path disabled on this hardware (needs KVM + >=4GB RAM); cannot launch {img}"
                )));
            }
            format!("qemu-system-x86_64 -enable-kvm -m 2048 -drive file={},format=raw", shq(img))
        }
    })
}

/// Resolve the launch command for a spec on the current machine (no spawn).
pub fn resolve(spec: &str) -> Result<String> {
    launch_command(&classify_target(spec), &Capabilities::detect())
}

/// Resolve and spawn the target detached; returns the command that was run.
pub fn launch(spec: &str) -> Result<String> {
    let cmd = resolve(spec)?;
    std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("setsid -f {cmd} >/dev/null 2>&1"))
        .spawn()
        .map_err(|e| Error::Msg(e.to_string()))?;
    Ok(cmd)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps(has_kvm: bool, ram_mb: u64) -> Capabilities {
        Capabilities { has_kvm, ram_mb }
    }

    #[test]
    fn routes_by_type() {
        assert!(matches!(classify_target("game.exe"), Target::WindowsExe(_)));
        assert!(matches!(classify_target("Tool.AppImage"), Target::AppImage(_)));
        assert!(matches!(classify_target("org.mozilla.firefox"), Target::Flatpak(_)));
        assert!(matches!(classify_target("disk.iso"), Target::Vm(_)));
        assert!(matches!(classify_target("htop"), Target::Pacman(_)));
    }

    #[test]
    fn builds_commands() {
        let c = caps(true, 8192);
        assert_eq!(launch_command(&Target::WindowsExe("a.exe".into()), &c).unwrap(), "wine 'a.exe'");
        assert_eq!(launch_command(&Target::Flatpak("org.x.Y".into()), &c).unwrap(), "flatpak run 'org.x.Y'");
        assert_eq!(launch_command(&Target::Pacman("htop".into()), &c).unwrap(), "'htop'");
        assert!(launch_command(&Target::AppImage("a.AppImage".into()), &c).unwrap().contains("chmod +x"));
    }

    #[test]
    fn vm_gated_by_hardware() {
        assert!(launch_command(&Target::Vm("x.iso".into()), &caps(true, 3700)).is_err()); // 3GB target
        assert!(launch_command(&Target::Vm("x.iso".into()), &caps(false, 16000)).is_err()); // no kvm
        assert!(launch_command(&Target::Vm("x.iso".into()), &caps(true, 8192))
            .unwrap()
            .contains("qemu-system"));
    }

    #[test]
    fn quotes_prevent_injection() {
        let cmd =
            launch_command(&Target::WindowsExe("a; rm -rf /.exe".into()), &caps(true, 8192)).unwrap();
        assert!(cmd.starts_with("wine '"));
        assert!(!cmd.contains("'; rm")); // cannot break out of the quotes
    }
}
