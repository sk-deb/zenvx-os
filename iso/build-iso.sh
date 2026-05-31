#!/usr/bin/env bash
# Build the ZenvX OS P1 ISO by overlaying our stack onto the Arch releng profile.
# Requires: archiso + root. Heavy (downloads packages, needs several GB + time).
set -euo pipefail
cd "$(dirname "$0")"

command -v mkarchiso >/dev/null || { echo "install archiso first: sudo pacman -S archiso"; exit 1; }

RELENG=/usr/share/archiso/configs/releng
rm -rf build-profile iso-work
cp -r "$RELENG" build-profile

# --- packages ---
cat overlay/packages.x86_64 >> build-profile/packages.x86_64

# --- airootfs overlay (autostart script + .zprofile) ---
cp -r overlay/airootfs/* build-profile/airootfs/
rm -f build-profile/airootfs/root/.bash_profile   # zsh ignores it; avoid confusion

# --- make our binaries executable in the FINAL image (archiso resets perms here) ---
{
  echo ''
  echo 'file_permissions+=('
  for b in zenvx zenvx-tui zenvx-repl ai-core zenvx-launch zenvx-voice zenvx-settings zenvx-compositor zenvx-start zenvx-say zenvx-listen whisper-cli; do
    echo "  [\"/usr/local/bin/$b\"]=\"0:0:755\""
  done
  echo ')'
} >> build-profile/profiledef.sh

# --- bundle local speech-to-text: build whisper.cpp + fetch the tiny English model ---
# (non-fatal: if it fails, voice output still works and push-to-talk degrades gracefully)
if command -v cmake >/dev/null && command -v git >/dev/null; then
  (
    set -e
    WD=$(mktemp -d)
    git clone --depth=1 https://github.com/ggml-org/whisper.cpp "$WD/wc"
    cmake -S "$WD/wc" -B "$WD/wc/build" -DCMAKE_BUILD_TYPE=Release -DWHISPER_BUILD_TESTS=OFF -DWHISPER_BUILD_EXAMPLES=ON
    cmake --build "$WD/wc/build" --target whisper-cli -j"$(nproc)"
    install -Dm755 "$WD/wc/build/bin/whisper-cli" build-profile/airootfs/usr/local/bin/whisper-cli
    mkdir -p build-profile/airootfs/usr/share/zenvx/whisper
    curl -L -o build-profile/airootfs/usr/share/zenvx/whisper/ggml-tiny.en.bin \
      https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin
    rm -rf "$WD"
    echo "whisper.cpp STT bundled."
  ) || echo "WARN: whisper.cpp build/download failed — push-to-talk will be disabled in this image."
else
  echo "WARN: cmake/git missing — skipping local STT (install: pacman -S cmake git)."
fi

# --- single boot entry: "ZenvX OS P1" ---
# BIOS (syslinux)
sed -i 's/^MENU TITLE .*/MENU TITLE ZenvX OS P1/' build-profile/syslinux/archiso_head.cfg
sed -i 's/^DEFAULT .*/DEFAULT zenvx/; s/^TIMEOUT .*/TIMEOUT 30/' build-profile/syslinux/archiso_sys.cfg
cat > build-profile/syslinux/archiso_sys-linux.cfg <<'EOF'
LABEL zenvx
MENU LABEL ZenvX OS P1
LINUX /%INSTALL_DIR%/boot/%ARCH%/vmlinuz-linux
INITRD /%INSTALL_DIR%/boot/%ARCH%/initramfs-linux.img
APPEND archisobasedir=%INSTALL_DIR% archisosearchuuid=%ARCHISO_UUID%
EOF

# UEFI (systemd-boot) — keep only one entry, titled "ZenvX OS P1"
rm -f build-profile/efiboot/loader/entries/02-archiso-speech-linux.conf \
      build-profile/efiboot/loader/entries/03-archiso-memtest86+x64.conf
cat > build-profile/efiboot/loader/entries/01-archiso-linux.conf <<'EOF'
title    ZenvX OS P1
sort-key 01
linux    /%INSTALL_DIR%/boot/%ARCH%/vmlinuz-linux
initrd   /%INSTALL_DIR%/boot/%ARCH%/initramfs-linux.img
options  archisobasedir=%INSTALL_DIR% archisosearchuuid=%ARCHISO_UUID%
EOF
cat > build-profile/efiboot/loader/loader.conf <<'EOF'
timeout 3
default 01-archiso-linux.conf
EOF

# --- build the Rust stack and stage binaries ---
( cd .. && cargo build --release )
for b in zenvx zenvx-tui zenvx-repl ai-core zenvx-launch zenvx-voice zenvx-settings zenvx-compositor; do
  install -Dm755 "../target/release/$b" "build-profile/airootfs/usr/local/bin/$b"
done

mkarchiso -v -w ./iso-work -o ./iso-out build-profile
echo "ISO written to iso/iso-out/"
