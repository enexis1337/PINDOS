// Запуск ELF процесса в PINDOS
//
// Схема:
//   1. Загружаем ELF → новый page directory
//   2. Устанавливаем IDT обработчик int 0x80
//   3. Переключаемся на page directory процесса
//   4. Прыгаем в user space через IRET (ring 3)
//   5. При int 0x80 → обработчик → syscall::handle
//   6. При exit → восстанавливаем ядро

use crate::vga;
// ИСПРАВЛЕНО: убран неиспользуемый LoadedElf
use crate::linux::elf;
use crate::linux::syscall::{self, SyscallRegs, ProcessState};
use crate::linux::paging::PageDir;
use crate::fs;

// GDT сегменты для user space (ring 3)
const USER_CS: u32 = 0x1B; // 0x18 | 3
const USER_DS: u32 = 0x23; // 0x20 | 3
const KERN_CS: u32 = 0x08;
const KERN_DS: u32 = 0x10;

// Глобальное состояние текущего процесса
static mut PROC_STATE: Option<ProcessState> = None;

// Сохранённый ESP ядра для возврата после завершения процесса
static mut KERNEL_RETURN_ESP: u32 = 0;

static mut PROC_EXITED: bool = false;
static mut PROC_EXIT_CODE: i32 = 0;

pub fn run(filename: &str) -> i32 {
    // Читаем файл из FS
    let file = match fs::get(filename) {
        Some(f) if !f.is_dir() => f,
        Some(_) => { vga::print("exec: is a directory\n"); return -1; }
        None    => { vga::print("exec: file not found\n"); return -1; }
    };

    let data = file.content_str().as_bytes();

    // Загружаем ELF
    let loaded = match elf::load(data) {
        Ok(l) => l,
        Err(e) => {
            vga::print("exec: ELF load error: ");
            vga::print(match e {
                elf::ElfError::TooSmall      => "file too small",
                elf::ElfError::BadMagic      => "not an ELF file",
                elf::ElfError::NotExecutable => "not an executable",
                elf::ElfError::NotI386       => "not i386 (need 32-bit binary)",
                elf::ElfError::NoPhdrs       => "no program headers",
                elf::ElfError::AllocFail     => "out of memory",
                elf::ElfError::BadSegment    => "bad segment",
            });
            vga::put_char(b'\n');
            return -1;
        }
    };

    vga::print("exec: loading ");
    vga::print(filename);
    vga::print(" entry=0x");
    print_hex(loaded.entry);
    vga::put_char(b'\n');

    // Инициализируем paging если ещё не было
    crate::linux::paging::ensure_init();

    // Инициализируем состояние процесса
    unsafe {
        PROC_STATE = Some(ProcessState::new(loaded.brk, loaded.brk_start));
        PROC_EXITED = false;
        PROC_EXIT_CODE = 0;
        setup_int80_handler();
        setup_gdt_user_segments();
    }

    // Строим начальный стек (argc=0, argv=NULL, envp=NULL)
    let stack_ptr = setup_stack(loaded.stack_top);

    // Активируем page directory процесса
    loaded.page_dir.activate();

    // Прыгаем в user space
    let exit_code = jump_to_userspace(loaded.entry, stack_ptr);

    // Восстанавливаем ядро
    PageDir::restore_kernel();

    unsafe { PROC_STATE = None; }

    exit_code
}

fn setup_stack(stack_top: u32) -> u32 {
    // Кладём на стек: argc=0, argv=0, envp=0
    let sp = stack_top - 16;
    unsafe {
        let ptr = sp as *mut u32;
        *ptr.add(0) = 0; // argc
        *ptr.add(1) = 0; // argv
        *ptr.add(2) = 0; // envp
        *ptr.add(3) = 0; // aux
    }
    sp
}

fn jump_to_userspace(entry: u32, stack: u32) -> i32 {
    unsafe {
        // Сохраняем ESP ядра для возврата из linux_syscall_dispatch
        core::arch::asm!(
            "mov [{0}], esp",
            in(reg) &raw mut KERNEL_RETURN_ESP,
        );

        // Переходим в ring 3 через IRET
        // Стек для IRET: SS, ESP, EFLAGS, CS, EIP
        core::arch::asm!(
            // Загружаем user data сегменты
            "mov ax, {user_ds}",
            "mov ds, ax",
            "mov es, ax",
            "mov fs, ax",
            "mov gs, ax",
            // Строим IRET фрейм
            "push {user_ds}",     // SS
            "push {stack:e}",     // ESP
            "push 0x202",         // EFLAGS: IF=1, reserved=1
            "push {user_cs}",     // CS
            "push {entry:e}",     // EIP
            "iretd",
            user_ds = const USER_DS,
            user_cs = const USER_CS,
            stack   = in(reg) stack,
            entry   = in(reg) entry,
            options(noreturn)
        );
    }
}

/// Обработчик int 0x80 — вызывается из user space
// ИСПРАВЛЕНО: naked_asm! вместо asm!
#[unsafe(naked)]
unsafe extern "C" fn int80_handler() {
    core::arch::naked_asm!(
        // Сохраняем сегменты и регистры
        "push gs",
        "push fs",
        "push es",
        "push ds",
        "pusha",
        // Загружаем kernel сегменты
        "mov ax, 0x10",
        "mov ds, ax",
        "mov es, ax",
        "mov fs, ax",
        "mov gs, ax",
        // Вызываем Rust обработчик
        "mov eax, esp",
        "push eax",
        "call linux_syscall_dispatch",
        "add esp, 4",
        // Восстанавливаем
        "popa",
        "pop ds",
        "pop es",
        "pop fs",
        "pop gs",
        "iretd",
    );
}

#[no_mangle]
unsafe extern "C" fn linux_syscall_dispatch(regs: *mut SyscallRegs) {
    if let Some(ref mut state) = PROC_STATE {
        // ИСПРАВЛЕНО: используем syscall через use в импортах
        syscall::handle(&mut *regs, state);

        if state.exited {
            let code = state.exit_code;
            PageDir::restore_kernel();
            PROC_EXIT_CODE = code;
            PROC_EXITED = true;

            // ИСПРАВЛЕНО: возвращаемся на сохранённый ESP ядра
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
}

fn setup_int80_handler() {
    // Устанавливаем IDT[0x80] — trap gate, DPL=3 (доступен из user space)
    unsafe {
        let mut idtr = [0u32; 2];
        core::arch::asm!("sidt [{0}]", in(reg) idtr.as_mut_ptr());
        let idt_base = idtr[1];
        let idt = idt_base as *mut u64;

        let handler = int80_handler as u32;
        let sel: u32 = KERN_CS;
        // Trap gate: type=0xEF (trap, DPL=3, 32-bit)
        let desc: u64 = ((handler as u64 & 0xFFFF0000) << 32)
            | (0xEF00u64 << 32)
            | ((sel as u64) << 16)
            | (handler as u64 & 0xFFFF);
        *idt.add(0x80) = desc;
    }
}

fn setup_gdt_user_segments() {
    // Добавляем user code (0x18) и user data (0x20) в GDT
    unsafe {
        let mut gdtr = [0u32; 2];
        core::arch::asm!("sgdt [{0}]", in(reg) gdtr.as_mut_ptr());
        let gdt_base = gdtr[1];
        let gdt = gdt_base as *mut u64;

        // User code: 0x18, DPL=3, 32-bit, execute/read
        *gdt.add(3) = 0x00CFFA000000FFFFu64;
        // User data: 0x20, DPL=3, 32-bit, read/write
        *gdt.add(4) = 0x00CFF2000000FFFFu64;
    }
}

fn print_hex(n: u32) {
    let digits = b"0123456789ABCDEF";
    for i in (0..8).rev() {
        let d = (n >> (i * 4)) & 0xF;
        vga::put_char(digits[d as usize]);
    }
}
