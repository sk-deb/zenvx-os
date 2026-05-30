#!/usr/bin/env bash
# Build the ZenvX OS ISO by overlaying our stack onto the Arch releng profile.
# Requires: archiso + root. Heavy (downloads packages, needs several GB + time).
set -euo pipefail
cd "$(dirname "$0")"

command -v mkarchiso >/dev/null || { echo "install archiso first: sudo pacman -S archiso"; exit 1; }

RELENG=/usr/share/archiso/configs/releng
rm -rf build-profile && cp -r "$RELENG" build-profile

# overlay packages + airootfs (autostart script, .bash_profile)
cat overlay/packages.x86_64 >> build-profile/packages.x86_64
cp -r overlay/airootfs/* build-profile/airootfs/
chmod +x build-profile/airootfs/usr/local/bin/zenvx-start

# build the Rust stack (release) and stage the binaries into the image
( cd .. && cargo build --release )
for b in zenvx zenvx-tui zenvx-repl ai-core zenvx-launch zenvx-voice zenvx-settings zenvx-compositor; do
  install -Dm755 "../target/release/$b" "build-profile/airootfs/usr/local/bin/$b"
done

sudo mkarchiso -v -w ./iso-work -o ./iso-out build-profile
echo "ISO written to iso/iso-out/"
