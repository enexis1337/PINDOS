use crate::kprintln;
use crate::mm::userptr::validate_user_slice;
use crate::sched::task::AddressSpace;

/// Saved user RSP during syscall entry (SYSCALL does NOT switch stacks).
pub static mut SC_RSP_SAVE: u64 = 0;
/// Kernel stack RSP used by syscall_entry.
pub static mut SC_KERNEL_RSP: u64 = 0;

/// Ошибки syscall операций
#[derive(Debug, Clone, Copy)]
pub enum SyscallError {
    BadAddress,
    BadFileDescriptor,
    InvalidArgument,
    NotImplemented,
}

impl From<crate::mm::UserPtrError> for SyscallError {
    fn from(err: crate::mm::UserPtrError) -> Self {
        match err {
            crate::mm::UserPtrError::BadAddress => SyscallError::BadAddress,
            crate::mm::UserPtrError::NotMapped => SyscallError::BadAddress,
            crate::mm::UserPtrError::NotWritable => SyscallError::BadAddress,
            crate::mm::UserPtrError::OverflowDetected => SyscallError::InvalidArgument,
        }
    }
}

impl From<SyscallError> for i64 {
    fn from(err: SyscallError) -> i64 {
        match err {
            SyscallError::BadAddress => -14,         // EFAULT
            SyscallError::BadFileDescriptor => -9,   // EBADF
            SyscallError::InvalidArgument => -22,    // EINVAL
            SyscallError::NotImplemented => -38,     // ENOSYS
        }
    }
}

// MSR адреса для SYSCALL/SYSRET
const MSR_EFER: u32 = 0xC0000080;      // Extended Feature Enable Register
const MSR_STAR: u32 = 0xC0000081;      // Selectors and Entry Point (CS/SS for syscall/sysret)
const MSR_LSTAR: u32 = 0xC0000082;     // Long mode syscall entry point (RIP)
const MSR_SFMASK: u32 = 0xC0000084;    // System call flag mask

// Биты EFER
const EFER_SCE: u64 = 1 << 0;          // System Call Extensions

// Маска для SFMASK — маскировать IF (interrupt flag, бит 9)
const SFMASK_IF: u64 = 1 << 9;

use crate::arch::gdt;

/// Установить kernel stack для syscall_entry
pub fn set_kernel_stack(rsp: u64) {
    unsafe { SC_KERNEL_RSP = rsp; }
}

/// Инициализация SYSCALL/SYSRET механизма
pub fn init() {
    unsafe {
        // 1. Включить SCE бит в MSR_EFER
        let mut efer = rdmsr(MSR_EFER);
        efer |= EFER_SCE;
        wrmsr(MSR_EFER, efer);

        // 2. Установить селекторы в MSR_STAR
        // биты 47:32 = KERNEL_CODE (0x08) — CS при входе в syscall
        // биты 63:48 = USER_CODE - 16 (0x18 | 3 = 0x1B, USER_CODE-16 = 0x1B-16 = 0x0B)
        let kernel_code = gdt::KERNEL_CODE as u64;
        let user_code_sel = (gdt::USER_CODE as u64) - 16;
        let star = (user_code_sel << 48) | (kernel_code << 32);
        wrmsr(MSR_STAR, star);

        // 3. Установить точку входа в MSR_LSTAR
        let syscall_entry_ptr = syscall_entry as *const () as u64;
        wrmsr(MSR_LSTAR, syscall_entry_ptr);

        // 4. Установить маску флагов (маскировать IF при входе в syscall)
        wrmsr(MSR_SFMASK, SFMASK_IF);
    }
}

/// Чтение из Model Specific Register
#[inline]
unsafe fn rdmsr(msr: u32) -> u64 {
    let high: u32;
    let low: u32;
    unsafe {
        core::arch::asm!(
            "rdmsr",
            in("ecx") msr,
            out("eax") low,
            out("edx") high,
            options(nostack, preserves_flags)
        );
    }
    ((high as u64) << 32) | (low as u64)
}

/// Запись в Model Specific Register
#[inline]
unsafe fn wrmsr(msr: u32, value: u64) {
    let high = (value >> 32) as u32;
    let low = value as u32;
    unsafe {
        core::arch::asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") low,
            in("edx") high,
            options(nostack, preserves_flags)
        );
    }
}

/// Точка входа из userspace — голый asm без Rust пролога.
/// При входе:
///   RCX = RIP возврата
///   R11 = RFLAGS
///   RAX = номер syscall
///   RDI, RSI, RDX = аргументы
///   R8, R9 = аргументы 4,5 (стандарт System V AMD64 ABI)
///
/// ВАЖНО: SYSCALL не переключает стек! RSP всё ещё указывает на user-стек.
/// Мы должны вручную сохранить user RSP и переключиться на kernel-стек.
#[unsafe(naked)]
unsafe extern "C" fn syscall_entry() {
    core::arch::naked_asm!(
        // 1. Save user RSP (SYSCALL doesn't switch it), then switch to kernel stack
        "mov [rip + {saved}], rsp",
        "mov rsp, [rip + {krsp}]",
        // 2. Push saved context onto kernel stack
        "push rcx",                     // RIP возврата
        "push r11",                     // RFLAGS возврата
        "push rdi",
        "push rsi",
        "push rdx",
        "push r8",
        "push r9",
        // 3. Reload user args from stack: stack[rsp+0..48] = r9 r8 rdx rsi rdi r11 rcx
        "mov rdi, rax",
        "mov rsi, [rsp + 32]",          // a0 = user's RDI (1st arg)
        "mov rdx, [rsp + 24]",          // a1 = user's RSI (2nd arg)
        "mov rcx, [rsp + 16]",          // a2 = user's RDX (3rd arg)
        "mov r8,  [rsp + 8]",           // a3 = user's R8  (4th arg)
        "mov r9,  [rsp + 0]",           // a4 = user's R9  (5th arg)
        "call {dispatch}",
        // 4. Restore registers
        "pop r9",
        "pop r8",
        "pop rdx",
        "pop rsi",
        "pop rdi",
        "pop r11",
        "pop rcx",
        // 5. Restore user RSP and return
        "mov rsp, [rip + {saved}]",
        "sysretq",
        saved = sym SC_RSP_SAVE,
        krsp = sym SC_KERNEL_RSP,
        dispatch = sym syscall_dispatch,
    );
}

/// Rust диспетчер syscall
/// Сигнатура для x86-64 SysV ABI: syscall_dispatch(nr, arg0, arg1, arg2, arg3, arg4)
#[no_mangle]
pub extern "C" fn syscall_dispatch(nr: u64, a0: u64, a1: u64, a2: u64, _a3: u64, _a4: u64) -> i64 {
    unsafe { crate::drivers::serial::SERIAL.get().write_byte(b'!'); }
    match nr {
        1 => sys_write(a0, a1, a2),
        60 => sys_exit(a0 as i32),
        _ => {
            kprintln!("[SYSCALL] unknown syscall: {}", nr);
            -38 // -ENOSYS
        }
    }
}

/// exit(code) — завершить процесс
fn sys_exit(code: i32) -> i64 {
    kprintln!("[USERSPACE exit({})] Halting", code);
    loop {
        unsafe {
            core::arch::asm!("hlt", options(nostack, preserves_flags));
        }
    }
}

/// Singleton for the current address space (single AS for now).
static ACTIVE_ASPACE: AddressSpace = AddressSpace;

/// write(fd, buf, count) — вывести данные на serial
fn sys_write(fd: u64, buf_ptr: u64, len: u64) -> i64 {
    if fd != 1 {
        return -9;
    }

    let slice = match validate_user_slice(&ACTIVE_ASPACE, buf_ptr, len) {
        Ok(s) => s,
        Err(_) => return -14, // -EFAULT
    };

    let prefix = b"[USERSPACE] ";
    unsafe {
        for &b in prefix {
            crate::drivers::serial::SERIAL.get().write_byte(b);
        }
    }
    for &b in slice {
        unsafe { crate::drivers::serial::SERIAL.get().write_byte(b); }
    }

    len as i64
}

/// Прыжок в userspace через SYSRET.
/// Устанавливает RCX=RIP, R11=RFLAGS, RSP=user_stack и выполняет sysretq.
pub unsafe fn jump_to_userspace(entry: u64, stack: u64) -> ! {
    // Write 'J' to COM1 just before sysretq
    unsafe { crate::drivers::serial::SERIAL.get().write_byte(b'J'); }
    unsafe {
        core::arch::asm!(
            "mov rcx, {entry}",
            "mov r11, {rflags}",
            "mov rsp, {stack}",
            "xor rbp, rbp",
            "sysretq",
            entry = in(reg) entry,
            rflags = in(reg) 0x3202u64,  // IF + IOPL=3 (userspace I/O)
            stack = in(reg) stack,
            options(noreturn)
        )
    }
}
