// Linux i386 syscall эмуляция через int 0x80
//
// Регистры при int 0x80:
//   EAX = номер syscall
//   EBX, ECX, EDX, ESI, EDI, EBP = аргументы
//   Возврат: EAX = результат (отрицательный = errno)

use crate::vga;
use crate::fs;

// Номера syscall (Linux i386)
const SYS_EXIT:     u32 = 1;
const SYS_FORK:     u32 = 2;
const SYS_READ:     u32 = 3;
const SYS_WRITE:    u32 = 4;
const SYS_OPEN:     u32 = 5;
const SYS_CLOSE:    u32 = 6;
const SYS_WAITPID:  u32 = 7;
const SYS_UNLINK:   u32 = 10;
const SYS_EXECVE:   u32 = 11;
const SYS_LSEEK:    u32 = 19;
const SYS_GETPID:   u32 = 20;
const SYS_GETUID:   u32 = 24;
const SYS_RENAME:   u32 = 38;
const SYS_MKDIR:    u32 = 39;
const SYS_RMDIR:    u32 = 40;
const SYS_DUP:      u32 = 41;
const SYS_TIMES:    u32 = 43;
const SYS_BRK:      u32 = 45;
const SYS_GETGID:   u32 = 47;
const SYS_GETEUID:  u32 = 49;
const SYS_GETEGID:  u32 = 50;
const SYS_IOCTL:    u32 = 54;
const SYS_DUP2:     u32 = 63;
const SYS_GETPPID:  u32 = 64;
const SYS_GETPGRP:  u32 = 65;
const SYS_SETSID:   u32 = 66;
const SYS_SIGACTION:u32 = 67;
const SYS_UNAME:    u32 = 122;
const SYS_MMAP:     u32 = 90;
const SYS_MUNMAP:   u32 = 91;
const SYS_STAT:     u32 = 106;
const SYS_FSTAT:    u32 = 108;
const SYS_GETCWD:   u32 = 183;
const SYS_EXIT_GROUP: u32 = 252;

// errno коды
const ENOENT:  i32 = -2;
const EBADF:   i32 = -9;
const ENOMEM:  i32 = -12;
const EACCES:  i32 = -13;
const EFAULT:  i32 = -14;
const ENOTDIR: i32 = -20;
const EINVAL:  i32 = -22;
const ENOSYS:  i32 = -38;

// Файловые дескрипторы процесса
const MAX_FDS: usize = 32;

pub struct FdTable {
    fds: [Fd; MAX_FDS],
}

#[derive(Copy, Clone)]
enum Fd {
    Closed,
    Stdin,
    Stdout,
    Stderr,
    File { name: [u8; 64], name_len: usize, offset: usize },
}

impl FdTable {
    pub fn new() -> Self {
        let mut t = FdTable { fds: [Fd::Closed; MAX_FDS] };
        t.fds[0] = Fd::Stdin;
        t.fds[1] = Fd::Stdout;
        t.fds[2] = Fd::Stderr;
        t
    }

    fn alloc(&mut self) -> Option<usize> {
        for (i, fd) in self.fds.iter().enumerate() {
            if matches!(fd, Fd::Closed) && i >= 3 {
                return Some(i);
            }
        }
        None
    }
}

pub struct ProcessState {
    pub fds:     FdTable,
    pub brk:     u32,
    pub brk_start: u32,
    pub pid:     u32,
    pub exited:  bool,
    pub exit_code: i32,
}

impl ProcessState {
    pub fn new(brk: u32, brk_start: u32) -> Self {
        ProcessState {
            fds: FdTable::new(),
            brk,
            brk_start,
            pid: 1,
            exited: false,
            exit_code: 0,
        }
    }
}

/// Регистры при int 0x80 (из стека)
#[repr(C)]
pub struct SyscallRegs {
    pub gs:  u32, pub fs: u32, pub es: u32, pub ds: u32,
    pub edi: u32, pub esi: u32, pub ebp: u32, pub esp: u32,
    pub ebx: u32, pub edx: u32, pub ecx: u32, pub eax: u32,
    pub eip: u32, pub cs:  u32, pub eflags: u32,
    pub user_esp: u32, pub ss: u32,
}

pub fn handle(regs: &mut SyscallRegs, state: &mut ProcessState) {
    let nr  = regs.eax;
    let ebx = regs.ebx;
    let ecx = regs.ecx;
    let edx = regs.edx;

    let result: i32 = match nr {
        SYS_EXIT | SYS_EXIT_GROUP => {
            state.exited = true;
            state.exit_code = ebx as i32;
            0
        }

        SYS_READ => {
            let fd  = ebx as usize;
            let buf = ecx as *mut u8;
            let len = edx as usize;
            sys_read(&mut state.fds, fd, buf, len)
        }

        SYS_WRITE => {
            let fd  = ebx as usize;
            let buf = ecx as *const u8;
            let len = edx as usize;
            sys_write(&state.fds, fd, buf, len)
        }

        SYS_OPEN => {
            let path = read_cstring(ebx);
            let flags = ecx;
            sys_open(&mut state.fds, path, flags)
        }

        SYS_CLOSE => {
            let fd = ebx as usize;
            if fd < MAX_FDS {
                state.fds.fds[fd] = Fd::Closed;
                0
            } else { EBADF }
        }

        SYS_BRK => {
            // brk(0) возвращает текущий brk
            // brk(addr) устанавливает новый brk
            if ebx == 0 {
                state.brk as i32
            } else if ebx >= state.brk_start {
                // Выделяем страницы если нужно
                let new_brk = (ebx + 0xFFF) & 0xFFFFF000;
                state.brk = new_brk;
                new_brk as i32
            } else {
                state.brk as i32
            }
        }

        SYS_MMAP => {
            // Упрощённый mmap — просто выделяем из brk
            let length = ecx;
            let aligned = (length + 0xFFF) & 0xFFFFF000;
            let addr = state.brk;
            state.brk += aligned;
            addr as i32
        }

        SYS_MUNMAP => 0, // игнорируем

        SYS_GETPID  => state.pid as i32,
        SYS_GETPPID => 0,
        SYS_GETUID  => 0,
        SYS_GETGID  => 0,
        SYS_GETEUID => 0,
        SYS_GETEGID => 0,
        SYS_GETPGRP => state.pid as i32,
        SYS_SETSID  => state.pid as i32,

        SYS_UNAME => {
            // struct utsname: sysname, nodename, release, version, machine (65 байт каждое)
            let ptr = ecx as *mut u8;
            unsafe {
                write_cstring(ptr,        b"PINDOS");
                write_cstring(ptr.add(65), b"pindos");
                write_cstring(ptr.add(130), b"0.1.0");
                write_cstring(ptr.add(195), b"#1 PINDOS");
                write_cstring(ptr.add(260), b"i686");
            }
            0
        }

        SYS_GETCWD => {
            let buf = ebx as *mut u8;
            let size = ecx as usize;
            let cwd = fs::cwd();
            let b = cwd.as_bytes();
            let len = b.len().min(size - 1);
            unsafe {
                for i in 0..len { *buf.add(i) = b[i]; }
                *buf.add(len) = 0;
            }
            ebx as i32
        }

        SYS_UNLINK => {
            let path = read_cstring(ebx);
            if fs::delete(path) { 0 } else { ENOENT }
        }

        SYS_MKDIR => {
            let path = read_cstring(ebx);
            if fs::mkdir(path) { 0 } else { ENOENT }
        }

        SYS_RMDIR => {
            let path = read_cstring(ebx);
            if fs::delete(path) { 0 } else { ENOENT }
        }

        SYS_RENAME => {
            let old = read_cstring(ebx);
            let new = read_cstring(ecx);
            if fs::rename(old, new) { 0 } else { ENOENT }
        }

        SYS_STAT | SYS_FSTAT => {
            // struct stat упрощённый — заполняем нулями
            let buf = ecx as *mut u8;
            unsafe {
                for i in 0..144usize { *buf.add(i) = 0; }
            }
            0
        }

        SYS_IOCTL => {
            // Игнорируем большинство ioctl
            // TIOCGWINSZ (0x5413) — размер терминала
            if ecx == 0x5413 {
                let ws = edx as *mut u16;
                unsafe {
                    *ws.add(0) = 25; // rows
                    *ws.add(1) = 80; // cols
                    *ws.add(2) = 0;
                    *ws.add(3) = 0;
                }
                0
            } else {
                EINVAL
            }
        }

        SYS_DUP => {
            let fd = ebx as usize;
            if fd < MAX_FDS {
                if let Some(new_fd) = state.fds.alloc() {
                    state.fds.fds[new_fd] = state.fds.fds[fd];
                    new_fd as i32
                } else { EBADF }
            } else { EBADF }
        }

        SYS_DUP2 => {
            let old = ebx as usize;
            let new = ecx as usize;
            if old < MAX_FDS && new < MAX_FDS {
                state.fds.fds[new] = state.fds.fds[old];
                new as i32
            } else { EBADF }
        }

        SYS_LSEEK => {
            let fd = ebx as usize;
            if fd < MAX_FDS {
                if let Fd::File { ref mut offset, name, name_len } = state.fds.fds[fd] {
                    let off = ecx as i32;
                    let whence = edx;
                    match whence {
                        0 => { *offset = off as usize; *offset as i32 } // SEEK_SET
                        1 => { *offset = (*offset as i32 + off) as usize; *offset as i32 } // SEEK_CUR
                        2 => {
                            let n = core::str::from_utf8(&name[..name_len]).unwrap_or("");
                            if let Some(f) = fs::get(n) {
                                *offset = (f.content_len as i32 + off) as usize;
                                *offset as i32
                            } else { ENOENT }
                        }
                        _ => EINVAL
                    }
                } else { EBADF }
            } else { EBADF }
        }

        SYS_SIGACTION => 0, // игнорируем сигналы
        SYS_TIMES     => 0,
        SYS_FORK      => EINVAL, // не поддерживаем fork
        SYS_EXECVE    => EINVAL, // не поддерживаем execve из процесса

        _ => {
            // Неизвестный syscall
            ENOSYS
        }
    };

    regs.eax = result as u32;
}

// ── Вспомогательные функции ───────────────────────────────────────────────

fn sys_write(fds: &FdTable, fd: usize, buf: *const u8, len: usize) -> i32 {
    if fd >= MAX_FDS { return EBADF; }
    match fds.fds[fd] {
        Fd::Stdout | Fd::Stderr => {
            // Выводим в VGA
            unsafe {
                for i in 0..len {
                    let c = *buf.add(i);
                    vga::put_char(c);
                }
            }
            len as i32
        }
        Fd::File { name, name_len, offset } => {
            let fname = core::str::from_utf8(&name[..name_len]).unwrap_or("");
            unsafe {
                let slice = core::slice::from_raw_parts(buf, len);
                let s = core::str::from_utf8(slice).unwrap_or("");
                fs::append(fname, s);
            }
            len as i32
        }
        _ => EBADF
    }
}

fn sys_read(fds: &mut FdTable, fd: usize, buf: *mut u8, len: usize) -> i32 {
    if fd >= MAX_FDS { return EBADF; }
    match fds.fds[fd] {
        Fd::Stdin => {
            // Читаем с клавиатуры
            let mut count = 0;
            unsafe {
                while count < len {
                    let c = vga::read_char();
                    *buf.add(count) = c;
                    vga::put_char(c);
                    count += 1;
                    if c == b'\n' { break; }
                }
            }
            count as i32
        }
        Fd::File { name, name_len, ref mut offset } => {
            let fname = core::str::from_utf8(&name[..name_len]).unwrap_or("");
            if let Some(f) = fs::get(fname) {
                let content = f.content_str().as_bytes();
                let start = *offset;
                let avail = content.len().saturating_sub(start);
                let n = avail.min(len);
                unsafe {
                    for i in 0..n {
                        *buf.add(i) = content[start + i];
                    }
                }
                *offset += n;
                n as i32
            } else { ENOENT }
        }
        _ => EBADF
    }
}

fn sys_open(fds: &mut FdTable, path: &str, flags: u32) -> i32 {
    const O_CREAT:  u32 = 0o100;
    const O_TRUNC:  u32 = 0o1000;
    const O_WRONLY: u32 = 0o1;
    const O_RDWR:   u32 = 0o2;

    // Создаём файл если O_CREAT
    if flags & O_CREAT != 0 {
        if fs::get(path).is_none() {
            fs::create(path, "");
        }
    }
    if flags & O_TRUNC != 0 {
        fs::write(path, "");
    }

    if fs::get(path).is_none() {
        return ENOENT;
    }

    if let Some(new_fd) = fds.alloc() {
        let mut name_arr = [0u8; 64];
        let b = path.as_bytes();
        let len = b.len().min(64);
        name_arr[..len].copy_from_slice(&b[..len]);
        fds.fds[new_fd] = Fd::File {
            name: name_arr,
            name_len: len,
            offset: 0,
        };
        new_fd as i32
    } else {
        EBADF
    }
}

fn read_cstring(addr: u32) -> &'static str {
    unsafe {
        let ptr = addr as *const u8;
        let mut len = 0;
        while *ptr.add(len) != 0 && len < 256 { len += 1; }
        core::str::from_utf8(core::slice::from_raw_parts(ptr, len)).unwrap_or("")
    }
}

unsafe fn write_cstring(dst: *mut u8, s: &[u8]) {
    for (i, &b) in s.iter().enumerate() {
        *dst.add(i) = b;
    }
    *dst.add(s.len()) = 0;
}
