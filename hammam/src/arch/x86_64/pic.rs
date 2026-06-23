// hammam/src/arch/x86_64/pic.rs
//
// Перепрограммирование 8259 PIC (Programmable Interrupt Controller).
// По умолчанию после BIOS/GRUB boot PIC шлёт IRQ0-7 на векторы 0x08-0x0F
// и IRQ8-15 на векторы 0x70-0x77 — это конфликтует с нашими CPU exception
// векторами (0x00-0x1F), особенно 0x08 = Double Fault.
// Remap: IRQ0-7 -> векторы 0x20-0x27, IRQ8-15 -> векторы 0x28-0x2F.

const PIC1_CMD: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_CMD: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;

const PIC1_OFFSET: u8 = 0x20;
const PIC2_OFFSET: u8 = 0x28;

unsafe fn outb(port: u16, val: u8) {
    // SAFETY: caller must ensure port safety and that IRQs are masked.
    unsafe { core::arch::asm!("out dx, al", in("dx") port, in("al") val, options(nostack)); }
}

unsafe fn inb(port: u16) -> u8 {
    // SAFETY: caller must ensure port safety and that IRQs are masked.
    let val: u8;
    unsafe { core::arch::asm!("in al, dx", in("dx") port, out("al") val, options(nostack)); }
    val
}

/// Перепрограммировать оба PIC на безопасные векторы (0x20-0x2F).
/// Вызывать один раз при инициализации ядра, до включения прерываний (sti).
pub fn remap() {
    unsafe {
        let mask1 = inb(PIC1_DATA);
        let mask2 = inb(PIC2_DATA);

        // ICW1: начать инициализацию, режим cascade
        outb(PIC1_CMD, 0x11);
        outb(PIC2_CMD, 0x11);

        // ICW2: задать базовый вектор
        outb(PIC1_DATA, PIC1_OFFSET);
        outb(PIC2_DATA, PIC2_OFFSET);

        // ICW3: cascade (PIC2 подключён к IRQ2 master'а)
        outb(PIC1_DATA, 0x04);
        outb(PIC2_DATA, 0x02);

        // ICW4: режим 8086
        outb(PIC1_DATA, 0x01);
        outb(PIC2_DATA, 0x01);

        // Восстановить маски
        outb(PIC1_DATA, mask1);
        outb(PIC2_DATA, mask2);
    }
}

/// Замаскировать все IRQ на обоих PIC — мы используем Local APIC timer,
/// legacy PIC не нужен.
pub fn disable() {
    unsafe {
        outb(PIC1_DATA, 0xFF);
        outb(PIC2_DATA, 0xFF);
    }
}

/// Подтвердить обработку IRQ (EOI).
#[allow(dead_code)]
pub fn send_eoi(irq: u8) {
    unsafe {
        if irq >= 8 {
            outb(PIC2_CMD, 0x20);
        }
        outb(PIC1_CMD, 0x20);
    }
}
