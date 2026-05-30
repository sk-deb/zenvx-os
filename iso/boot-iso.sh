#!/usr/bin/env bash
# Boot the built ZenvX ISO in QEMU and assert the session reaches the shell.
set -euo pipefail
cd "$(dirname "$0")"

ISO=$(ls -t iso-out/*.iso 2>/dev/null | head -1 || true)
[ -n "$ISO" ] || { echo "no ISO found — run ./build-iso.sh first"; exit 1; }

echo "booting $ISO ..."
timeout 180 qemu-system-x86_64 -enable-kvm -m 2048 -cdrom "$ISO" \
  -nographic -serial mon:stdio 2>&1 | tee /tmp/zenvx-boot.log || true

if grep -q "ZenvX session ready" /tmp/zenvx-boot.log; then
  echo "BOOT OK: session reached the shell"
else
  echo "BOOT FAIL: session banner not seen"; exit 1
fi
