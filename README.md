# ZenvX OS

A voice-driven, Arch-based Linux session: an ~80% GUI zone that composites any
app (Windows `.exe` via Wine, Linux via Flatpak/AppImage/pacman) and a ~20%
terminal pane running an always-on LLM agent that can operate the system behind
a safety gate. Online-first (OpenRouter), with local fallback (Ollama), and
graceful degradation on low-end hardware.

> Target machine: Pentium 3825U / ~3.7GB RAM. Design is online-first; heavy local
> models and the VM path auto-disable on weak hardware and unlock on capable machines.

## Architecture (Rust workspace)

| Crate | Role | Status |
|---|---|---|
| `common` | shared types, errors, secure config (provider/model/key, 0600) | ✅ tested |
| `ai-core` | one streaming `LlmProvider` trait; OpenRouter (SSE) + Ollama (ndjson) | ✅ tested |
| `agent` | tool-calling (`run_shell`/`open_app`/`list_files`) + **safety gate** | ✅ tested |
| `repl` | terminal LLM REPL: streaming chat + inline, gated tool calls | ✅ tested |
| `launcher` | compatibility router (.exe→Wine, AppImage, flatpak, pacman, VM) | ✅ tested |
| `voice` | wake→STT→agent→TTS pipeline (whisper.cpp/Piper), degrades if absent | ✅ tested |
| `mode` | hardware/network detection → online-first / local / VM policy | ✅ tested |
| `compositor` | 80/20 zone layout + surface tiling (Smithay loop plugs in) | ✅ tested |
| `shell-ui` | settings state + key masking (native/Tauri overlay binds to it) | ✅ tested |
| `session` | wires voice → agent → launcher → UI → compositor; embeds the REPL | ✅ tested |

## Safety

Shell commands are classified: **catastrophic** (e.g. `rm -rf /`, `mkfs`) are
hard-denied and never executed; **destructive/root** (e.g. `sudo …`) require
**two** confirmations; everything else runs. App names are single-quoted to
prevent shell injection. API keys live in `~/.config/zenvx/config` (mode 0600),
never in the repo, and are masked in the UI.

## Build & run

```sh
cargo build              # build the whole workspace
cargo test               # run all tests

cargo run -p zenvx                       # first-boot provider setup
cargo run -p zenvx-ai-core -- ask "hi"   # stream a reply from the active provider
cargo run -p zenvx-repl                  # interactive agent REPL
cargo run -p zenvx-launch -- --dry game.exe   # show how a target would launch
```

### See it boot in QEMU (no flashing)

```sh
make run        # boot the ZenvX boot screen in a QEMU window
make verify     # headless: boot + assert the screen renders
```

### Build the bootable ISO (needs `archiso` + root)

```sh
cd iso && ./build-iso.sh   # overlays the stack onto Arch releng, runs mkarchiso
./boot-iso.sh              # boots the ISO in QEMU and asserts the session starts
```

## Notes on the heavy pieces

- **Compositor** (`compositor`) ships the tested layout engine; the Smithay
  Wayland event loop runs in a graphical session and configures surfaces with
  these rectangles.
- **Shell UI** (`shell-ui`) ships the tested settings/persistence backend; the
  visual overlay (native layer-shell, or Tauri) renders it. On the 3GB target a
  native overlay is preferred over Tauri/WebKit to save RAM.
- **Voice** degrades to typed input when whisper.cpp/Piper aren't installed.
