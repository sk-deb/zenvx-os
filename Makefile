CC      := gcc
LD      := ld
CFLAGS  := -m32 -ffreestanding -fno-stack-protector -fno-pic -nostdlib -Wall -Wextra -O2
LDFLAGS := -m elf_i386 -T linker.ld
QEMU    := qemu-system-i386
QFLAGS  := -m 256M

KERNEL  := zenvx.elf

all: $(KERNEL)

boot.o: boot.S
	$(CC) $(CFLAGS) -c $< -o $@

kernel.o: kernel.c
	$(CC) $(CFLAGS) -c $< -o $@

$(KERNEL): boot.o kernel.o linker.ld
	$(LD) $(LDFLAGS) -o $@ boot.o kernel.o

# Live window: edit -> make run -> watch it boot. No ISO, no flashing.
# GDK_BACKEND=wayland: this machine runs Plasma Wayland; avoids the Xwayland auth issue.
run: $(KERNEL)
	GDK_BACKEND=wayland $(QEMU) -kernel $(KERNEL) $(QFLAGS) -display gtk

# Headless verify: boot, screendump the VGA output, assert it's not blank.
verify: $(KERNEL)
	@{ sleep 2; printf 'screendump /tmp/zenvx.ppm\n'; sleep 1; printf 'quit\n'; } | \
	  timeout 10 $(QEMU) -kernel $(KERNEL) $(QFLAGS) -display none -monitor stdio >/dev/null 2>&1; \
	  python3 -c "import re,sys;d=open('/tmp/zenvx.ppm','rb').read();m=re.match(rb'P6\s+(\d+)\s+(\d+)\s+(\d+)\s',d);px=d[m.end():];nb=sum(1 for i in range(0,len(px)-2,3) if (px[i],px[i+1],px[i+2])!=(0,0,0));print('nonblack pixels:',nb);sys.exit(0 if nb>0 else 1)" \
	  && echo "VERIFY OK: ZenvX boots and renders" || echo "VERIFY FAIL: blank screen"

# Optional: real GRUB ISO (closer to the eventual distro boot path).
iso: $(KERNEL) grub.cfg
	mkdir -p isodir/boot/grub
	cp $(KERNEL) isodir/boot/$(KERNEL)
	cp grub.cfg isodir/boot/grub/grub.cfg
	grub-mkrescue -o zenvx.iso isodir
run-iso: iso
	$(QEMU) -cdrom zenvx.iso $(QFLAGS)

clean:
	rm -f *.o $(KERNEL) zenvx.iso
	rm -rf isodir

.PHONY: all run verify iso run-iso clean
