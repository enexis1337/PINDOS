use crate::kprintln;
use crate::mm::userptr::validate_user_slice;
use crate::sched::task::AddressSpace;
use crate::process::{PROCESS_TABLE, CURRENT_PROCESS, next_pid, Process};
use crate::vfs::VFS;
use alloc::vec;

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
            SyscallError::BadAddress => -14,
            SyscallError::BadFileDescriptor => -9,
            SyscallError::InvalidArgument => -22,
            SyscallError::NotImplemented => -38,
        }
    }
}

// MSR адреса для SYSCALL/SYSRET
const MSR_EFER: u32 = 0xC0000080;
const MSR_STAR: u32 = 0xC0000081;
const MSR_LSTAR: u32 = 0xC0000082;
const MSR_SFMASK: u32 = 0xC0000084;

// Биты EFER
const EFER_SCE: u64 = 1 << 0;

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
        let mut efer = rdmsr(MSR_EFER);
        efer |= EFER_SCE;
        wrmsr(MSR_EFER, efer);

        let kernel_code = gdt::KERNEL_CODE as u64;
        let user_code_sel = (gdt::USER_CODE as u64) - 16;
        let star = (user_code_sel << 48) | (kernel_code << 32);
        wrmsr(MSR_STAR, star);

        let syscall_entry_ptr = syscall_entry as *const () as u64;
        wrmsr(MSR_LSTAR, syscall_entry_ptr);

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
#[unsafe(naked)]
unsafe extern "C" fn syscall_entry() {
    core::arch::naked_asm!(
        "mov [rip + {saved}], rsp",
        "mov rsp, [rip + {krsp}]",
        "push rcx",
        "push r11",
        "push rdi",
        "push rsi",
        "push rdx",
        "push r8",
        "push r9",
        "mov rdi, rax",
        "mov rsi, [rsp + 32]",
        "mov rdx, [rsp + 24]",
        "mov rcx, [rsp + 16]",
        "mov r8,  [rsp + 8]",
        "mov r9,  [rsp + 0]",
        "call {dispatch}",
        "pop r9",
        "pop r8",
        "pop rdx",
        "pop rsi",
        "pop rdi",
        "pop r11",
        "pop rcx",
        "mov rsp, [rip + {saved}]",
        "sysretq",
        saved = sym SC_RSP_SAVE,
        krsp = sym SC_KERNEL_RSP,
        dispatch = sym syscall_dispatch,
    );
}

/// Rust диспетчер syscall
#[no_mangle]
pub extern "C" fn syscall_dispatch(nr: u64, a0: u64, a1: u64, a2: u64, _a3: u64, _a4: u64) -> i64 {
    unsafe { crate::drivers::serial::SERIAL.get().write_byte(b'!'); }
    match nr {
        1 => sys_write(a0, a1, a2),
        2 => sys_exec(a0, a1),
        3 => sys_waitpid(a0, a1),
        60 => sys_exit(a0 as i32),
        _ => {
            kprintln!("[SYSCALL] unknown syscall: {}", nr);
            -38
        }
    }
}

/// exit(code) — завершить процесс
fn sys_exit(code: i32) -> i64 {
    kprintln!("[syscall] exit({})", code);
    if let Some(proc) = CURRENT_PROCESS.lock().as_ref() {
        let pid = proc.pid;
        if let Some(p) = PROCESS_TABLE.lock().get(&pid) {
            p.exit_code.store(code, Ordering::Release);
            p.is_zombie.store(true, Ordering::Release);
        }
    }
    crate::sched::exit_current();
    unreachable!()
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
        Err(_) => return -14,
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

/// exec(path) — запустить новый процесс
fn sys_exec(path_ptr: u64, path_len: u64) -> i64 {
    let path_bytes = match validate_user_slice(&ACTIVE_ASPACE, path_ptr, path_len) {
        Ok(s) => s,
        Err(_) => return -14,
    };
    let path = match core::str::from_utf8(path_bytes) {
        Ok(s) => s,
        Err(_) => return -22,
    };

    kprintln!("[syscall] exec: {}", path);

    let vnode = match VFS.lock().lookup(path) {
        Ok(v) => v,
        Err(e) => {
            kprintln!("[syscall] exec: lookup failed: {:?}", e);
            return -2;
        }
    };

    let stat = match vnode.stat() {
        Ok(s) => s,
        Err(e) => {
            kprintln!("[syscall] exec: stat failed: {:?}", e);
            return -5;
        }
    };

    kprintln!("[syscall] exec: file size={}", stat.size);
    let mut elf_data = vec![0u8; stat.size as usize];
    if let Err(e) = vnode.read(0, &mut elf_data) {
        kprintln!("[syscall] exec: read failed: {:?}", e);
        return -5;
    }

    let process = match Process::from_elf(next_pid(), &elf_data) {
        Ok(p) => p,
        Err(e) => {
            kprintln!("[syscall] exec: from_elf failed: {:?}", e);
            return -12;
        }
    };

    let pid = process.pid;
    let process_arc = Arc::new(process);

    crate::sched::SCHEDULER.lock().add_task(process_arc.main_task.clone());
    PROCESS_TABLE.lock().insert(pid, Arc::clone(&process_arc));

    kprintln!("[syscall] exec: spawned pid={}", pid);
    pid as i64
}

/// waitpid(pid, flags) — ожидать завершения процесса
fn sys_waitpid(pid: u64, flags: u64) -> i64 {
    let wnohang = flags & 1 != 0;
    let pid = pid as u32;

    let table = PROCESS_TABLE.lock();
    match table.get(&pid) {
        None => -10,
        Some(proc) => {
            if proc.is_zombie() {
                let code = proc.exit_code.load(Ordering::Acquire);
                drop(table);
                PROCESS_TABLE.lock().remove(&pid);
                code as i64
            } else if wnohang {
                0
            } else {
                -11
            }
        }
    }
}

/// Прыжок в userspace через SYSRET.
pub unsafe fn jump_to_userspace(entry: u64, stack: u64) -> ! {
    unsafe {
        core::arch::asm!(
            "mov rcx, {entry}",
            "mov r11, {rflags}",
            "mov rsp, {stack}",
            "xor rbp, rbp",
            "sysretq",
            entry = in(reg) entry,
            rflags = in(reg) 0x3202u64,
            stack = in(reg) stack,
            options(noreturn)
        )
    }
}

use core::sync::atomic::Ordering;
use alloc::sync::Arc;