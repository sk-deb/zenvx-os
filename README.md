# ZenvX OS

A voice-driven, Arch-based Linux session: an ~80% GUI zone that composites any
app (Windows `.exe` via Wine, Linux via Flatpak/AppImage/pacman) and a ~20%
terminal pane running an always-on LLM agent that can operate the system behind
a safety gate. Online-first (OpenRouter), with local fallback (Ollama), and
graceful degradation on low-end hardware.

> Target machine: Pentium 3825U / ~3.7GB RAM. Design is online-first; heavy local
> models and the VM path auto-disable on weak hardware and unlock on capable machines.

---

## Quick Start

### Requirements

- **Arch Linux** (or any distro with `pacman` for building)
- `gcc`, `ld`, `grub-mkrescue`, `xorriso`, `mkfs.fat` (for the boot harness)
- `rust` (1.70+)
- `qemu` + `qemu-ui-gtk` (for testing)
- `archiso` (for building the bootable ISO)

### Install dependencies (Arch)

```sh
sudo pacman -S --needed base-devel rust qemu-system-x86 qemu-ui-gtk grub xorriso dosfstools archiso
```

---

## Build & Run (development)

```sh
git clone https://github.com/sk-deb/zenvx-os.git
cd zenvx-os

# Build the Rust workspace
cargo build --release

# Run tests (31 tests, all passing)
cargo test

# First-boot setup (choose OpenRouter or local Ollama)
cargo run -p zenvx

# Launch the TUI interface
cargo run -p zenvx-tui
```

### See it boot in QEMU (no ISO needed)

```sh
make run       # opens a QEMU window showing the ZenvX boot screen
make verify    # headless: boot + assert the screen renders
```

---

## Build the Bootable ISO

```sh
cd iso
./build-iso.sh
```

This:
1. Copies the Arch `releng` profile
2. Appends ZenvX packages (Wine, Flatpak, Ollama, Pipewire, etc.)
3. Builds the Rust workspace in release mode
4. Stages all binaries into the live image
5. Runs `mkarchiso` → outputs `iso/iso-out/*.iso`

---

## Flash to USB

**⚠️ This erases the target device. Double-check the device name.**

```sh
# Identify your USB (look for TYPE=disk, TRAN=usb, RM=1)
lsblk -o NAME,SIZE,TYPE,TRAN,RM

# Unmount any mounted partitions
sudo umount /dev/sdX?* 2>/dev/null

# Flash (replace /dev/sdX with your USB device)
sudo dd if=iso/iso-out/*.iso of=/dev/sdX bs=4M conv=fsync status=progress
sync
```

### Verify the flash

```sh
lsblk -o NAME,SIZE,LABEL /dev/sdX
# Should show: ARCH_YYYYMM label + ARCHISO_EFI partition
```

---

## Boot from USB

1. Plug the USB into the target machine
2. Enter the boot menu (F12 / F2 / Esc / Del at power-on)
3. Select the USB device
4. ZenvX auto-starts:
   - First boot: asks for your **OpenRouter API key** (or switch to local Ollama)
   - Then drops into the **ZenvX TUI** — full-screen chat + app launcher

### First boot options

| Choice | What happens |
|--------|-------------|
| Enter an OpenRouter key | Cloud AI (works on any hardware) |
| Skip → Yes to local | Uses Ollama (`llama3.2:1b` default — run `ollama pull llama3.2:1b` first) |
| Skip → No | Asks for the key again |

---

## Architecture

| Crate | Role |
|-------|------|
| `common` | Shared types, errors, secure config (0600 perms) |
| `ai-core` | Streaming `LlmProvider` trait; OpenRouter (SSE) + Ollama (ndjson) |
| `agent` | Tool-calling + **safety gate** (double-confirm for root, hard denylist) |
| `repl` | Terminal LLM REPL with inline tool execution |
| `launcher` | App compatibility router (.exe→Wine, AppImage, Flatpak, pacman, VM) |
| `voice` | Wake→STT→TTS pipeline (whisper.cpp/Piper); degrades if absent |
| `mode` | Hardware/network detection → online-first / local / VM policy |
| `compositor` | 80/20 zone layout + surface tiling engine |
| `shell-ui` | Settings state + API key masking |
| `session` | Wires voice → agent → launcher → UI → compositor |
| `tui` | Full-screen terminal interface (ratatui) |

---

## Safety

- **Catastrophic commands** (`rm -rf /`, `mkfs`, fork bombs) → **hard-denied, never executed**
- **Root/destructive commands** (`sudo`, `rm -r`, `shutdown`) → **double confirmation required**
- **App names** → single-quoted to prevent shell injection
- **API keys** → stored in `~/.config/zenvx/config` with mode `0600`, masked in UI, never in the repo

---

## TUI Commands

| Command | Action |
|---------|--------|
| (type normally) | Chat with the AI agent |
| `/launch <app>` | Open an app (routes through Wine/Flatpak/pacman) |
| `/quit` or `Esc` | Exit |

---

## Project Structure

```
zenvx-os/
├── Cargo.toml          # Workspace root
├── Makefile            # Boot harness (QEMU)
├── boot.S / kernel.c   # Minimal boot screen kernel
├── crates/
│   ├── common/         # Shared types + config
│   ├── ai-core/        # AI streaming adapter
│   ├── agent/          # Tool dispatch + safety gate
│   ├── repl/           # LLM REPL
│   ├── launcher/       # App compat router
│   ├── voice/          # Voice pipeline
│   ├── mode/           # Hardware detection + policy
│   ├── compositor/     # Zone layout engine
│   ├── shell-ui/       # Settings backend
│   ├── session/        # Full session orchestrator
│   └── tui/            # Terminal UI (ratatui)
├── iso/
│   ├── build-iso.sh    # Builds the Arch ISO
│   ├── boot-iso.sh     # QEMU boot test
│   └── overlay/        # Custom packages + autostart
└── README.md
```

---

## License

MIT
