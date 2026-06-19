use crate::kprintln;
use crate::mm::validate_user_slice;

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
#[unsafe(naked)]
unsafe extern "C" fn syscall_entry() {
    core::arch::naked_asm!(
        // Сохранить возврат userspace состояния
        "push rcx",                     // RIP возврата (будет восстановлен в sysretq)
        "push r11",                     // RFLAGS возврата (будет восстановлен в sysretq)
        
        // Сохранить регистры пользователя которые нужно восстановить
        "push rdi",
        "push rsi",
        "push rdx",
        "push r8",
        "push r9",
        
        // RAX содержит номер syscall, RDI-RDX уже позиционированы корректно
        // Вызвать Rust диспетчер: syscall_dispatch(rax, rdi, rsi, rdx, r8, r9)
        // x86-64 SysV ABI: rdi, rsi, rdx, rcx, r8, r9
        // RAX уже содержит номер syscall (перейдет в RDI)
        "mov rdi, rax",                 // номер syscall -> RDI
        // RSI, RDX, R8, R9 уже содержат аргументы
        "call {0}",
        
        // Восстановить регистры пользователя
        "pop r9",
        "pop r8",
        "pop rdx",
        "pop rsi",
        "pop rdi",
        
        // Восстановить RFLAGS и RIP, вернуться в userspace через sysretq
        "pop r11",                      // RFLAGS
        "pop rcx",                      // RIP
        "sysretq",
        
        sym syscall_dispatch,
    );
}

/// Rust диспетчер syscall
/// Сигнатура для x86-64 SysV ABI: syscall_dispatch(nr, arg0, arg1, arg2, arg3, arg4)
#[no_mangle]
pub extern "C" fn syscall_dispatch(nr: u64, a0: u64, a1: u64, a2: u64, _a3: u64, _a4: u64) -> i64 {
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

/// write(fd, buf, count) — вывести данные на serial
fn sys_write(fd: u64, buf_ptr: u64, len: u64) -> i64 {
    if fd != 1 {
        return -9;
    }

    let user_buffer = match validate_user_slice(buf_ptr, len) {
        Ok(buf) => buf,
        Err(e) => {
            let err: SyscallError = e.into();
            return i64::from(err);
        }
    };

    for &b in user_buffer {
        unsafe { crate::drivers::serial::SERIAL.get().write_byte(b); }
    }

    len as i64
}

/// Прыжок в userspace через SYSRET.
/// Устанавливает RCX=RIP, R11=RFLAGS, RSP=user_stack и выполняет sysretq.
pub unsafe fn jump_to_userspace(entry: u64, stack: u64) -> ! {
    unsafe {
        core::arch::asm!(
            "mov rcx, {entry}",
            "mov r11, {rflags}",
            "mov rsp, {stack}",
            "xor rbp, rbp",
            "sysretq",
            entry = in(reg) entry,
            rflags = in(reg) 0x202u64,
            stack = in(reg) stack,
            options(noreturn)
        )
    }
}
