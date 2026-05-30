#include <stdint.h>

/* --- VGA text mode (what you see in the QEMU window) --- */
static volatile uint16_t *const VGA = (uint16_t *)0xB8000;
enum { VGA_W = 80, VGA_H = 25 };

/* --- COM1 serial (used to verify boot headlessly) --- */
#define COM1 0x3F8
static inline void outb(uint16_t p, uint8_t v) { __asm__ volatile("outb %0,%1" ::"a"(v), "Nd"(p)); }
static inline uint8_t inb(uint16_t p) { uint8_t r; __asm__ volatile("inb %1,%0" : "=a"(r) : "Nd"(p)); return r; }

static void serial_init(void) {
    outb(COM1 + 1, 0x00); outb(COM1 + 3, 0x80); outb(COM1 + 0, 0x03);
    outb(COM1 + 1, 0x00); outb(COM1 + 3, 0x03); outb(COM1 + 2, 0xC7); outb(COM1 + 4, 0x0B);
}
static void serial_putc(char c) { while (!(inb(COM1 + 5) & 0x20)) {} outb(COM1, c); }
static void serial_puts(const char *s) { for (; *s; s++) serial_putc(*s); }

static void vga_clear(uint8_t attr) {
    for (int i = 0; i < VGA_W * VGA_H; i++) VGA[i] = (uint16_t)' ' | ((uint16_t)attr << 8);
}
static void vga_puts(int row, int col, uint8_t attr, const char *s) {
    int i = row * VGA_W + col;
    for (; *s; s++, i++) VGA[i] = (uint16_t)(uint8_t)*s | ((uint16_t)attr << 8);
}

void kernel_main(void) {
    serial_init();
    vga_clear(0x07); /* light grey on black */

    /* Bump this stage line as features land — proves the live update loop. */
    vga_puts(1, 2, 0x0A, "ZenvX OS");                         /* bright green */
    vga_puts(2, 2, 0x07, "voice-driven Arch shell (stage 0: boot harness)");
    vga_puts(4, 2, 0x08, "booted in QEMU via -kernel. no ISO, no flashing.");

    serial_puts("ZenvX OS stage 0: boot harness OK\n");

    for (;;) __asm__ volatile("hlt");
}
