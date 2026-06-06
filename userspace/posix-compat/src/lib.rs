#![no_std]
#![allow(non_camel_case_types)]

//! POSIX compatibility layer for Hammam syscalls
//!
//! Трансляция стандартных POSIX вызовов в native Hammam syscalls.
//! Позволяет портировать Linux программы с минимальными изменениями.

use core::ffi::{c_char, c_int, c_void};

// Стандартные errno коды
pub const EPERM: i32 = 1;   // Operation not permitted
pub const ENOENT: i32 = 2;  // No such file or directory
pub const ESRCH: i32 = 3;   // No such process
pub const EINTR: i32 = 4;   // Interrupted system call
pub const EIO: i32 = 5;     // Input/output error
pub const ENXIO: i32 = 6;   // No such device or address
pub const E2BIG: i32 = 7;   // Argument list too long
pub const ENOEXEC: i32 = 8; // Exec format error
pub const EBADF: i32 = 9;   // Bad file descriptor
pub const ENOMEM: i32 = 12; // Out of memory
pub const EACCES: i32 = 13; // Permission denied
pub const EFAULT: i32 = 14; // Bad address
pub const EBUSY: i32 = 16;  // Device or resource busy
pub const EEXIST: i32 = 17; // File exists
pub const EXDEV: i32 = 18;  // Invalid cross-device link
pub const ENODEV: i32 = 19; // No such device
pub const ENOTDIR: i32 = 20; // Not a directory
pub const EISDIR: i32 = 21; // Is a directory
pub const EINVAL: i32 = 22; // Invalid argument
pub const ENFILE: i32 = 23; // Too many open files in system
pub const EMFILE: i32 = 24; // Too many open files
pub const ENOSYS: i32 = 38; // Function not implemented
pub const EOVERFLOW: i32 = 75; // Value too large for defined data type

// Флаги для open()
pub const O_RDONLY: c_int = 0;
pub const O_WRONLY: c_int = 1;
pub const O_RDWR: c_int = 2;
pub const O_APPEND: c_int = 0x0400;
pub const O_CREAT: c_int = 0x0040;
pub const O_EXCL: c_int = 0x0080;
pub const O_TRUNC: c_int = 0x0200;

// Thread-local errno (используя глобальную переменную в no_std)
static mut ERRNO_VAL: i32 = 0;

/// Установить errno
#[inline]
unsafe fn set_errno(err: i32) {
    ERRNO_VAL = err;
}

/// Получить errno
#[inline]
pub unsafe fn get_errno() -> i32 {
    ERRNO_VAL
}

/// Сбросить errno
#[inline]
pub unsafe fn clear_errno() {
    ERRNO_VAL = 0;
}

// ===== Syscall numbers (System V AMD64 ABI) =====
// 0 = read
// 1 = write
// 2 = open
// 3 = close
// 60 = exit

/// Выполнить syscall write(2)
/// rax = 1, rdi = fd, rsi = buf, rdx = count
#[inline]
unsafe fn syscall_write(fd: i32, buf: *const u8, count: usize) -> isize {
    let ret: isize;
    core::arch::asm!(
        "syscall",
        in("rax") 1u64,
        in("rdi") fd as u64,
        in("rsi") buf as u64,
        in("rdx") count as u64,
        lateout("rax") ret,
        options(nostack)
    );
    ret
}

/// Выполнить syscall read(2)
/// rax = 0, rdi = fd, rsi = buf, rdx = count
#[inline]
unsafe fn syscall_read(fd: i32, buf: *mut u8, count: usize) -> isize {
    let ret: isize;
    core::arch::asm!(
        "syscall",
        in("rax") 0u64,
        in("rdi") fd as u64,
        in("rsi") buf as u64,
        in("rdx") count as u64,
        lateout("rax") ret,
        options(nostack)
    );
    ret
}

/// Выполнить syscall open(2) — TODO: требует расширения Hammam
/// Временно возвращает -ENOSYS
#[inline]
unsafe fn syscall_open(_path: *const u8, _flags: i32, _mode: i32) -> isize {
    // Hammam еще не поддерживает open syscall
    set_errno(ENOSYS);
    -1
}

/// Выполнить syscall close(2) — TODO: требует расширения Hammam
#[inline]
unsafe fn syscall_close(_fd: i32) -> isize {
    // Hammam еще не поддерживает close syscall
    set_errno(ENOSYS);
    -1
}

/// Выполнить syscall exit(2)
/// rax = 60, rdi = status
#[inline]
unsafe fn syscall_exit(status: i32) -> ! {
    core::arch::asm!(
        "syscall",
        in("rax") 60u64,
        in("rdi") status as u64,
        options(nostack, noreturn)
    )
}

// ===== POSIX API Implementation =====

/// POSIX open(2)
/// Открыть файл
#[no_mangle]
pub unsafe extern "C" fn open(path: *const c_char, flags: c_int) -> c_int {
    if path.is_null() {
        set_errno(EFAULT);
        return -1;
    }

    // Расчет mode из flags (для целей совместимости)
    let mode = 0o644; // Стандартный mode для create

    let ret = syscall_open(path as *const u8, flags, mode);
    if ret < 0 {
        -1
    } else {
        ret as c_int
    }
}

/// POSIX open(2) с mode параметром
#[no_mangle]
pub unsafe extern "C" fn open_mode(path: *const c_char, flags: c_int, mode: i32) -> c_int {
    if path.is_null() {
        set_errno(EFAULT);
        return -1;
    }

    let ret = syscall_open(path as *const u8, flags, mode);
    if ret < 0 {
        -1
    } else {
        ret as c_int
    }
}

/// POSIX read(2)
/// Прочитать данные из файлового дескриптора
#[no_mangle]
pub unsafe extern "C" fn read(fd: c_int, buf: *mut c_void, count: usize) -> isize {
    if fd < 0 {
        set_errno(EBADF);
        return -1;
    }

    if buf.is_null() {
        set_errno(EFAULT);
        return -1;
    }

    let ret = syscall_read(fd, buf as *mut u8, count);
    if ret < 0 {
        // Обработать ошибку syscall
        let err = (-ret) as i32;
        set_errno(err);
        -1
    } else {
        ret
    }
}

/// POSIX write(2)
/// Записать данные в файловый дескриптор
#[no_mangle]
pub unsafe extern "C" fn write(fd: c_int, buf: *const c_void, count: usize) -> isize {
    if fd < 0 {
        set_errno(EBADF);
        return -1;
    }

    if buf.is_null() {
        set_errno(EFAULT);
        return -1;
    }

    let ret = syscall_write(fd, buf as *const u8, count);
    if ret < 0 {
        // Обработать ошибку syscall
        let err = (-ret) as i32;
        set_errno(err);
        -1
    } else {
        ret
    }
}

/// POSIX close(2)
/// Закрыть файловый дескриптор
#[no_mangle]
pub unsafe extern "C" fn close(fd: c_int) -> c_int {
    if fd < 0 {
        set_errno(EBADF);
        return -1;
    }

    let ret = syscall_close(fd);
    if ret < 0 {
        -1
    } else {
        ret as c_int
    }
}

/// POSIX exit(2)
/// Завершить процесс
#[no_mangle]
pub unsafe extern "C" fn exit(status: c_int) -> ! {
    syscall_exit(status)
}

/// POSIX exit_group(2) — завершить всю группу процессов
#[no_mangle]
pub unsafe extern "C" fn exit_group(status: c_int) -> ! {
    syscall_exit(status)
}

// ===== Additional POSIX stubs =====

/// POSIX printf-like function (stub)
/// В реальном коде используется libc printf
#[no_mangle]
pub unsafe extern "C" fn puts(s: *const c_char) -> c_int {
    if s.is_null() {
        return -1;
    }

    // Найти длину строки
    let mut len = 0;
    let mut ptr = s as *const u8;
    while *ptr != 0 {
        len += 1;
        ptr = ptr.add(1);
    }

    // Использовать write syscall для вывода на stdout (fd=1)
    let ret = syscall_write(1, s as *const u8, len);
    if ret >= 0 {
        // Добавить newline
        syscall_write(1, b"\n".as_ptr(), 1);
        0
    } else {
        -1
    }
}

/// POSIX getpid(2) — получить PID процесса
/// Временно возвращает фиксированное значение
#[no_mangle]
pub unsafe extern "C" fn getpid() -> i32 {
    // TODO: требует расширения Hammam syscall
    1
}

/// POSIX getppid(2) — получить PID родительского процесса
#[no_mangle]
pub unsafe extern "C" fn getppid() -> i32 {
    0
}

/// POSIX fork(2) — создать дочерний процесс
/// Временно не поддерживается
#[no_mangle]
pub unsafe extern "C" fn fork() -> i32 {
    set_errno(ENOSYS);
    -1
}

/// POSIX execve(2) — заменить образ процесса
/// Временно не поддерживается
#[no_mangle]
pub unsafe extern "C" fn execve(
    _filename: *const c_char,
    _argv: *const *const c_char,
    _envp: *const *const c_char,
) -> c_int {
    set_errno(ENOSYS);
    -1
}

/// POSIX waitpid(2) — ждать завершения дочернего процесса
/// Временно не поддерживается
#[no_mangle]
pub unsafe extern "C" fn waitpid(_pid: i32, _wstatus: *mut c_int, _options: c_int) -> i32 {
    set_errno(ENOSYS);
    -1
}

/// POSIX usleep(3) — спать в микросекундах
/// Временно не поддерживается
#[no_mangle]
pub unsafe extern "C" fn usleep(_usecs: u32) -> c_int {
    0
}

/// POSIX sleep(3) — спать в секундах
/// Временно не поддерживается
#[no_mangle]
pub unsafe extern "C" fn sleep(_secs: u32) -> u32 {
    0
}

/// POSIX abort(3) — прервать программу
#[no_mangle]
pub unsafe extern "C" fn abort() -> ! {
    syscall_exit(134)
}

/// POSIX malloc(3) — выделить память
/// В no_std это не поддерживается напрямую
#[no_mangle]
pub unsafe extern "C" fn malloc(_size: usize) -> *mut c_void {
    // TODO: требует глобального аллокатора
    core::ptr::null_mut()
}

/// POSIX free(3) — освободить память
#[no_mangle]
pub unsafe extern "C" fn free(_ptr: *mut c_void) {
    // TODO: требует глобального аллокатора
}

/// POSIX memset(3) — заполнить память
#[no_mangle]
pub unsafe extern "C" fn memset(s: *mut c_void, c: c_int, n: usize) -> *mut c_void {
    if s.is_null() {
        return core::ptr::null_mut();
    }

    let ptr = s as *mut u8;
    let byte = c as u8;
    for i in 0..n {
        ptr.add(i).write_volatile(byte);
    }

    s
}

/// POSIX memcpy(3) — скопировать память
#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    if dest.is_null() || src.is_null() {
        return dest;
    }

    core::ptr::copy_nonoverlapping(src as *const u8, dest as *mut u8, n);
    dest
}

/// POSIX memmove(3) — переместить память (overlapping safe)
#[no_mangle]
pub unsafe extern "C" fn memmove(
    dest: *mut c_void,
    src: *const c_void,
    n: usize,
) -> *mut c_void {
    if dest.is_null() || src.is_null() {
        return dest;
    }

    core::ptr::copy(src as *const u8, dest as *mut u8, n);
    dest
}

/// POSIX strlen(3) — длина строки
#[no_mangle]
pub unsafe extern "C" fn strlen(s: *const c_char) -> usize {
    if s.is_null() {
        return 0;
    }

    let mut len = 0;
    let mut ptr = s as *const u8;
    while *ptr != 0 {
        len += 1;
        ptr = ptr.add(1);
    }
    len
}

/// POSIX strcmp(3) — сравнить строки
#[no_mangle]
pub unsafe extern "C" fn strcmp(s1: *const c_char, s2: *const c_char) -> c_int {
    if s1.is_null() || s2.is_null() {
        return 0;
    }

    let mut ptr1 = s1 as *const u8;
    let mut ptr2 = s2 as *const u8;

    loop {
        let c1 = *ptr1;
        let c2 = *ptr2;

        if c1 != c2 {
            return (c1 as c_int) - (c2 as c_int);
        }

        if c1 == 0 {
            return 0;
        }

        ptr1 = ptr1.add(1);
        ptr2 = ptr2.add(1);
    }
}

/// POSIX strncmp(3) — сравнить N символов строк
#[no_mangle]
pub unsafe extern "C" fn strncmp(s1: *const c_char, s2: *const c_char, n: usize) -> c_int {
    if s1.is_null() || s2.is_null() {
        return 0;
    }

    let mut ptr1 = s1 as *const u8;
    let mut ptr2 = s2 as *const u8;

    for _ in 0..n {
        let c1 = *ptr1;
        let c2 = *ptr2;

        if c1 != c2 {
            return (c1 as c_int) - (c2 as c_int);
        }

        if c1 == 0 {
            return 0;
        }

        ptr1 = ptr1.add(1);
        ptr2 = ptr2.add(1);
    }

    0
}

/// POSIX strcpy(3) — копировать строку (UNSAFE)
#[no_mangle]
pub unsafe extern "C" fn strcpy(dest: *mut c_char, src: *const c_char) -> *mut c_char {
    if dest.is_null() || src.is_null() {
        return dest;
    }

    let mut ptr_src = src as *const u8;
    let mut ptr_dest = dest as *mut u8;

    loop {
        let c = *ptr_src;
        ptr_dest.write_volatile(c);

        if c == 0 {
            break;
        }

        ptr_src = ptr_src.add(1);
        ptr_dest = ptr_dest.add(1);
    }

    dest
}

/// POSIX strncpy(3) — копировать N символов строки
#[no_mangle]
pub unsafe extern "C" fn strncpy(
    dest: *mut c_char,
    src: *const c_char,
    n: usize,
) -> *mut c_char {
    if dest.is_null() || src.is_null() {
        return dest;
    }

    let mut ptr_src = src as *const u8;
    let mut ptr_dest = dest as *mut u8;
    let mut i = 0;

    while i < n {
        let c = *ptr_src;
        ptr_dest.write_volatile(c);

        if c == 0 {
            // Pad with zeros до конца
            for j in (i + 1)..n {
                ptr_dest.add(j - i).write_volatile(0);
            }
            break;
        }

        ptr_src = ptr_src.add(1);
        ptr_dest = ptr_dest.add(1);
        i += 1;
    }

    dest
}

/// POSIX strerror(3) — получить описание ошибки
#[no_mangle]
pub unsafe extern "C" fn strerror(errnum: c_int) -> *const c_char {
    let msg: &[u8] = match errnum {
        EPERM => b"Operation not permitted\0",
        ENOENT => b"No such file or directory\0",
        ESRCH => b"No such process\0",
        EINTR => b"Interrupted system call\0",
        EIO => b"Input/output error\0",
        ENXIO => b"No such device or address\0",
        E2BIG => b"Argument list too long\0",
        ENOEXEC => b"Exec format error\0",
        EBADF => b"Bad file descriptor\0",
        ENOMEM => b"Out of memory\0",
        EACCES => b"Permission denied\0",
        EFAULT => b"Bad address\0",
        EBUSY => b"Device or resource busy\0",
        EEXIST => b"File exists\0",
        EXDEV => b"Invalid cross-device link\0",
        ENODEV => b"No such device\0",
        ENOTDIR => b"Not a directory\0",
        EISDIR => b"Is a directory\0",
        EINVAL => b"Invalid argument\0",
        ENFILE => b"Too many open files in system\0",
        EMFILE => b"Too many open files\0",
        ENOSYS => b"Function not implemented\0",
        EOVERFLOW => b"Value too large for defined data type\0",
        _ => b"Unknown error\0",
    };

    msg.as_ptr() as *const c_char
}
