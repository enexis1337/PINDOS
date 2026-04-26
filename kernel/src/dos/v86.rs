// Virtual 8086 Mode — запуск DOS .COM программ
//
// Схема:
//   1. Настраиваем TSS (Task State Segment) для обработки #GP из v86
//   2. Устанавливаем обработчик IDT для #GP (вектор 13)
//   3. Переходим в v86 через IRET с установленным VM-битом в EFLAGS
//   4. При INT xx из v86 → #GP → наш обработчик → эмулируем INT

use crate::vga;
use crate::dos::int21::{self, V86Regs, Int21Result};
use crate::dos::loader::PSP_SIZE;

// IDT — 256 дескрипторов по 8 байт
static mut IDT: [[u32; 2]; 256] = [[0u32; 2]; 256];

// Сохранённый ESP ядра — для возврата после завершения DOS-программы
static mut KERNEL_RETURN_ESP: u32 = 0;

// TSS для переключения стека при #GP из v86
#[repr(C, align(4))]
struct Tss {
    link: u16, _r0: u16,
    esp0: u32,
    ss0: u16, _r1: u16,
    esp1: u32, ss1: u16, _r2: u16,
    esp2: u32, ss2: u16, _r3: u16,
    cr3: u32,
    eip: u32, eflags: u32,
    eax: u32, ecx: u32, edx: u32, ebx: u32,
    esp: u32, ebp: u32, esi: u32, edi: u32,
    es: u16, _r4: u16,
    cs: u16, _r5: u16,
    ss: u16, _r6: u16,
    ds: u16, _r7: u16,
    fs: u16, _r8: u16,
    gs: u16, _r9: u16,
    ldt: u16, _r10: u16,
    trap: u16,
    iomap_base: u16,
}

static mut TSS: Tss = Tss {
    link: 0, _r0: 0,
    esp0: 0x8F000, // стек ядра для #GP
    ss0: 0x10,     // data сегмент ядра
    esp1: 0, ss1: 0, _r1: 0,
    esp2: 0, ss2: 0, _r2: 0,
    cr3: 0,
    eip: 0, eflags: 0,
    eax: 0, ecx: 0, edx: 0, ebx: 0,
    esp: 0, ebp: 0, esi: 0, edi: 0,
    es: 0, _r4: 0,
    cs: 0, _r5: 0,
    ss: 0, _r6: 0,
    ds: 0, _r7: 0,
    fs: 0, _r8: 0,
    gs: 0, _r9: 0,
    ldt: 0, _r10: 0,
    trap: 0,
    iomap_base: 0x68,
    _r3: 0,
};

// Флаг — программа завершилась
static mut DOS_TERMINATED: bool = false;

pub fn init() {
    setup_idt();
}

fn setup_idt() {
    unsafe {
        // Перемаппируем PIC: IRQ0-7 → векторы 0x20-0x27, IRQ8-15 → 0x28-0x2F
        // Это стандартное перемаппирование для защищённого режима.
        // Без него IRQ0 (таймер) попадает на вектор 8 (#DF), вызывая Triple Fault.
        remap_pic();

        // Заглушки для IRQ0-IRQ15 (векторы 0x20-0x2F) — просто EOI и iret
        for i in 0x20usize..=0x2F {
            set_idt_gate(i, irq_stub as u32, 0x08, 0x8E);
        }

        // Обработчик #GP (вектор 13)
        set_idt_gate(13, gp_handler as u32, 0x08, 0x8E);
        // INT 20h (вектор 0x20) — terminate (перекрываем заглушку)
        set_idt_gate(0x20, int20_handler as u32, 0x08, 0x8E);

        let limit = (core::mem::size_of_val(&IDT) - 1) as u16;
        let base = IDT.as_ptr() as u32;

        // IDTR: 6 байт — limit (2) + base (4)
        let idtr: [u8; 6] = [
            (limit & 0xFF) as u8,
            (limit >> 8) as u8,
            (base & 0xFF) as u8,
            ((base >> 8) & 0xFF) as u8,
            ((base >> 16) & 0xFF) as u8,
            ((base >> 24) & 0xFF) as u8,
        ];

        core::arch::asm!(
            "lidt [{0}]",
            in(reg) idtr.as_ptr(),
        );
    }
}

/// Перемаппирование 8259 PIC: IRQ0-7 → INT 0x20-0x27, IRQ8-15 → INT 0x28-0x2F
unsafe fn remap_pic() {
    // ICW1: начало инициализации
    x86_out(0x20, 0x11);
    x86_out(0xA0, 0x11);
    // ICW2: базовые векторы
    x86_out(0x21, 0x20); // мастер: IRQ0 → вектор 0x20
    x86_out(0xA1, 0x28); // слейв: IRQ8 → вектор 0x28
    // ICW3: каскадирование
    x86_out(0x21, 0x04);
    x86_out(0xA1, 0x02);
    // ICW4: режим 8086
    x86_out(0x21, 0x01);
    x86_out(0xA1, 0x01);
    // Маскируем все IRQ кроме IRQ0 (таймер) и IRQ1 (клавиатура)
    x86_out(0x21, 0xFC); // разрешаем IRQ0 и IRQ1
    x86_out(0xA1, 0xFF); // слейв — всё маскируем
}

unsafe fn x86_out(port: u16, val: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") val);
}

/// Заглушка для аппаратных IRQ — отправляет EOI и возвращается
#[unsafe(naked)]
unsafe extern "C" fn irq_stub() {
    core::arch::naked_asm!(
        "push eax",
        "mov al, 0x20",   // EOI мастеру
        "out 0x20, al",
        "pop eax",
        "iretd",
    );
}

fn set_idt_gate(vec: usize, handler: u32, sel: u16, flags: u8) {
    unsafe {
        IDT[vec][0] = (sel as u32) << 16 | (handler & 0xFFFF);
        IDT[vec][1] = (handler & 0xFFFF0000) | ((flags as u32) << 8);
    }
}

/// Запустить .COM программу в v86 режиме
pub fn run_com(segment: u32) {
    unsafe {
        DOS_TERMINATED = false;

        // Точка входа: segment:0x100 (после PSP)
        let entry_ip: u32 = PSP_SIZE;
        let entry_cs: u32 = segment;
        let entry_ss: u32 = segment;
        let entry_sp: u32 = 0xFFFE; // стек в конце сегмента

        // EFLAGS с VM=1 (бит 17) и IF=1 (бит 9)
        let eflags: u32 = (1 << 17) | (1 << 9) | 0x02;

        // Сохраняем текущий ESP ядра для возврата после завершения DOS-программы
        core::arch::asm!(
            "mov [{0}], esp",
            in(reg) &raw mut KERNEL_RETURN_ESP,
        );

        // Переходим в v86 через IRET
        // Стек для IRET в v86: GS, FS, DS, ES, SS, ESP, EFLAGS, CS, EIP
        core::arch::asm!(
            // Загружаем сегменты ядра
            "mov ax, 0x10",
            "mov ds, ax",
            "mov es, ax",
            // Строим фрейм для IRET в v86
            "push {gs:e}",      // GS
            "push {fs:e}",      // FS
            "push {ds_val:e}",  // DS
            "push {es_val:e}",  // ES
            "push {ss:e}",      // SS
            "push {esp:e}",     // ESP
            "push {eflags:e}",  // EFLAGS (VM=1)
            "push {cs:e}",      // CS
            "push {eip:e}",     // EIP
            "iretd",
            gs      = in(reg) entry_cs,
            fs      = in(reg) entry_cs,
            ds_val  = in(reg) entry_cs,
            es_val  = in(reg) entry_cs,
            ss      = in(reg) entry_ss,
            esp     = in(reg) entry_sp,
            eflags  = in(reg) eflags,
            cs      = in(reg) entry_cs,
            eip     = in(reg) entry_ip,
            options(noreturn)
        );
    }
}

/// Обработчик #GP — вызывается при INT xx из v86
#[unsafe(naked)]
unsafe extern "C" fn gp_handler() {
    // ИСПРАВЛЕНО: naked_asm! вместо asm! (требование Rust >= 1.88)
    core::arch::naked_asm!(
        "pusha",
        "push gs",
        "push fs",
        "push ds",
        "push es",
        // Передаём указатель на регистры
        "mov eax, esp",
        "push eax",
        "call v86_gp_dispatch",
        "add esp, 4",
        "pop es",
        "pop ds",
        "pop fs",
        "pop gs",
        "popa",
        "iretd",
    );
}

#[unsafe(naked)]
unsafe extern "C" fn int20_handler() {
    // ИСПРАВЛЕНО: naked_asm! вместо asm! (требование Rust >= 1.88)
    core::arch::naked_asm!(
        "pusha",
        "call v86_terminate",
        "popa",
        "iretd",
    );
}

#[no_mangle]
unsafe extern "C" fn v86_gp_dispatch(regs: *mut V86Regs) {
    let regs = &mut *regs;

    // Читаем байт инструкции из CS:IP
    let cs = regs.cs as u32;
    let ip = regs.eip & 0xFFFF;
    let instr_addr = (cs << 4) + ip;
    let opcode = *(instr_addr as *const u8);

    match opcode {
        // INT imm8
        0xCD => {
            let int_num = *((instr_addr + 1) as *const u8);
            regs.eip = (regs.eip & 0xFFFF0000) | ((ip + 2) & 0xFFFF);

            match int_num {
                0x20 => { DOS_TERMINATED = true; }
                0x21 => {
                    match int21::handle(regs) {
                        Int21Result::Terminate => { DOS_TERMINATED = true; }
                        Int21Result::Continue => {}
                    }
                }
                0x10 => { int21::handle_int10(regs); }
                0x16 => { int21::handle_int16(regs); }
                0x33 => { int21::handle_int33(regs); }
                // Остальные INT — игнорируем
                _ => {}
            }

            if DOS_TERMINATED {
                // ИСПРАВЛЕНО: возвращаемся на сохранённый ESP ядра,
                // а не на хардкод 0x8F000 без адреса возврата
                core::arch::asm!(
                    "mov esp, [{0}]",
                    "mov ax, 0x10",
                    "mov ds, ax",
                    "mov es, ax",
                    "mov fs, ax",
                    "mov gs, ax",
                    "ret",
                    in(reg) &raw const KERNEL_RETURN_ESP,
                    options(noreturn)
                );
            }
        }
        // CLI/STI — игнорируем в v86
        0xFA | 0xFB => {
            regs.eip = (regs.eip & 0xFFFF0000) | ((ip + 1) & 0xFFFF);
        }
        // IN/OUT — игнорируем
        0xE4 | 0xE5 | 0xE6 | 0xE7 |
        0xEC | 0xED | 0xEE | 0xEF => {
            regs.eip = (regs.eip & 0xFFFF0000) | ((ip + 2) & 0xFFFF);
        }
        _ => {
            vga::print("[v86 #GP: unknown opcode]\n");
            DOS_TERMINATED = true;
        }
    }
}

#[no_mangle]
unsafe extern "C" fn v86_terminate() {
    DOS_TERMINATED = true;
}