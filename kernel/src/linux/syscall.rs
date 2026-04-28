// Linux i386 syscall эмуляция — расширенная версия
// int 0x80: EAX=nr, EBX/ECX/EDX/ESI/EDI/EBP=args, возврат EAX

use crate::vga;
use crate::fs;

// ── Номера syscall ────────────────────────────────────────────────────────
const SYS_EXIT:        u32 = 1;
const SYS_FORK:        u32 = 2;
const SYS_READ:        u32 = 3;
const SYS_WRITE:       u32 = 4;
const SYS_OPEN:        u32 = 5;
const SYS_CLOSE:       u32 = 6;
const SYS_WAITPID:     u32 = 7;
const SYS_CREAT:       u32 = 8;
const SYS_LINK:        u32 = 9;
const SYS_UNLINK:      u32 = 10;
const SYS_EXECVE:      u32 = 11;
const SYS_CHDIR:       u32 = 12;
const SYS_CHMOD:       u32 = 15;  // POSIX
const SYS_CHOWN:       u32 = 16;  // POSIX (16-bit uid)
const SYS_LSEEK:       u32 = 19;
const SYS_GETPID:      u32 = 20;
const SYS_GETUID:      u32 = 24;
const SYS_SIGNAL:      u32 = 48;  // POSIX signal()
const SYS_ACCESS:      u32 = 33;
const SYS_KILL:        u32 = 37;
const SYS_RENAME:      u32 = 38;
const SYS_MKDIR:       u32 = 39;
const SYS_RMDIR:       u32 = 40;
const SYS_DUP:         u32 = 41;
const SYS_PIPE:        u32 = 42;
const SYS_TIMES:       u32 = 43;
const SYS_BRK:         u32 = 45;
const SYS_GETGID:      u32 = 47;
const SYS_GETEUID:     u32 = 49;
const SYS_GETEGID:     u32 = 50;
const SYS_IOCTL:       u32 = 54;
const SYS_FCNTL:       u32 = 55;
const SYS_SETPGID:     u32 = 57;
const SYS_UMASK:       u32 = 60;
const SYS_DUP2:        u32 = 63;
const SYS_GETPPID:     u32 = 64;
const SYS_GETPGRP:     u32 = 65;
const SYS_SETSID:      u32 = 66;
const SYS_SIGACTION:   u32 = 67;
const SYS_SETREUID:    u32 = 70;
const SYS_SETREGID:    u32 = 71;
const SYS_SIGSUSPEND:  u32 = 72;
const SYS_SIGPENDING:  u32 = 73;
const SYS_SETHOSTNAME: u32 = 74;
const SYS_SETRLIMIT:   u32 = 75;
const SYS_GETRLIMIT:   u32 = 76;
const SYS_GETRUSAGE:   u32 = 77;
const SYS_GETTIMEOFDAY:u32 = 78;
const SYS_SYMLINK:     u32 = 83;
const SYS_READLINK:    u32 = 85;
const SYS_MMAP:        u32 = 90;
const SYS_MUNMAP:      u32 = 91;
const SYS_TRUNCATE:    u32 = 92;
const SYS_FTRUNCATE:   u32 = 93;
const SYS_FCHMOD:      u32 = 94;
const SYS_FCHOWN:      u32 = 95;
const SYS_GETPRIORITY: u32 = 96;
const SYS_SETPRIORITY: u32 = 97;
const SYS_STAT:        u32 = 106;
const SYS_LSTAT:       u32 = 107;
const SYS_FSTAT:       u32 = 108;
const SYS_FSYNC:       u32 = 118;
const SYS_UNAME:       u32 = 122;
const SYS_MPROTECT:    u32 = 125;
const SYS_SIGPROCMASK: u32 = 126;
const SYS_GETDENTS:    u32 = 141;
const SYS_GETCWD:      u32 = 183;
const SYS_MMAP2:       u32 = 192;
const SYS_STAT64:      u32 = 195;
const SYS_LSTAT64:     u32 = 196;
const SYS_FSTAT64:     u32 = 197;
const SYS_GETUID32:    u32 = 199;
const SYS_GETGID32:    u32 = 200;
const SYS_GETEUID32:   u32 = 201;
const SYS_GETEGID32:   u32 = 202;
const SYS_GETDENTS64:  u32 = 220;
const SYS_FCNTL64:     u32 = 221;
const SYS_EXIT_GROUP:  u32 = 252;
const SYS_SET_TID_ADDR:u32 = 258;
const SYS_CLOCK_GETTIME:u32= 265;
const SYS_OPENAT:      u32 = 295;
const SYS_MKDIRAT:     u32 = 296;
const SYS_UNLINKAT:    u32 = 301;
const SYS_RENAMEAT:    u32 = 302;
const SYS_FACCESSAT:   u32 = 307;
const SYS_SET_ROBUST_LIST: u32 = 311;

// ── errno ─────────────────────────────────────────────────────────────────
const EPERM:   i32 = -1;
const ENOENT:  i32 = -2;
const EINTR:   i32 = -4;
const EBADF:   i32 = -9;
const ENOMEM:  i32 = -12;
const EACCES:  i32 = -13;
const EFAULT:  i32 = -14;
const EBUSY:   i32 = -16;
const EEXIST:  i32 = -17;
const ENOTDIR: i32 = -20;
const EISDIR:  i32 = -21;
const EINVAL:  i32 = -22;
const EMFILE:  i32 = -24;
const ENOSPC:  i32 = -28;
const EPIPE:   i32 = -32;
const ENOSYS:  i32 = -38;

// ── Файловые дескрипторы ──────────────────────────────────────────────────
pub const MAX_FDS: usize = 64;

#[derive(Copy, Clone)]
pub enum Fd {
    Closed,
    Stdin,
    Stdout,
    Stderr,
    File { name: [u8; 128], name_len: usize, offset: usize, flags: u32 },
    PipeRead  { buf: [u8; 4096], len: usize, pos: usize },
    PipeWrite { peer: usize },
}

pub struct FdTable {
    pub fds: [Fd; MAX_FDS],
}

impl FdTable {
    pub fn new() -> Self {
        let mut t = FdTable { fds: [Fd::Closed; MAX_FDS] };
        t.fds[0] = Fd::Stdin;
        t.fds[1] = Fd::Stdout;
        t.fds[2] = Fd::Stderr;
        t
    }

    pub fn alloc_from(&mut self, start: usize) -> Option<usize> {
        for i in start..MAX_FDS {
            if matches!(self.fds[i], Fd::Closed) {
                return Some(i);
            }
        }
        None
    }

    pub fn alloc(&mut self) -> Option<usize> {
        self.alloc_from(3)
    }
}

// ── Состояние процесса ────────────────────────────────────────────────────
pub struct ProcessState {
    pub fds:       FdTable,
    pub brk:       u32,
    pub brk_start: u32,
    pub pid:       u32,
    pub exited:    bool,
    pub exit_code: i32,
    pub umask:     u32,
    pub sighandlers: [u32; 32], // адреса обработчиков сигналов (0 = SIG_DFL)
}

impl ProcessState {
    pub fn new(brk: u32, brk_start: u32) -> Self {
        ProcessState {
            fds: FdTable::new(),
            brk, brk_start,
            pid: 1,
            exited: false,
            exit_code: 0,
            umask: 0o022,
            sighandlers: [0u32; 32],
        }
    }
}

// ── Регистры ──────────────────────────────────────────────────────────────
#[repr(C)]
pub struct SyscallRegs {
    pub gs: u32, pub fs: u32, pub es: u32, pub ds: u32,
    pub edi: u32, pub esi: u32, pub ebp: u32, pub esp: u32,
    pub ebx: u32, pub edx: u32, pub ecx: u32, pub eax: u32,
    pub eip: u32, pub cs: u32, pub eflags: u32,
    pub user_esp: u32, pub ss: u32,
}

// ── Главный диспетчер ─────────────────────────────────────────────────────
pub fn handle(regs: &mut SyscallRegs, state: &mut ProcessState) {
    let nr  = regs.eax;
    let ebx = regs.ebx;
    let ecx = regs.ecx;
    let edx = regs.edx;
    let esi = regs.esi;

    let result: i32 = match nr {
        // ── Процесс ───────────────────────────────────────────────────────
        SYS_EXIT | SYS_EXIT_GROUP => {
            state.exited = true;
            state.exit_code = ebx as i32;
            0
        }
        SYS_FORK      => EPERM,  // не поддерживаем
        SYS_EXECVE    => EPERM,
        SYS_WAITPID   => EPERM,
        SYS_GETPID    => state.pid as i32,
        SYS_GETPPID   => 0,
        SYS_GETUID | SYS_GETUID32   => 0,
        SYS_GETGID | SYS_GETGID32   => 0,
        SYS_GETEUID | SYS_GETEUID32 => 0,
        SYS_GETEGID | SYS_GETEGID32 => 0,
        SYS_GETPGRP   => state.pid as i32,
        SYS_SETPGID   => 0,
        SYS_SETSID    => state.pid as i32,
        SYS_SETREUID | SYS_SETREGID => 0,
        SYS_KILL      => 0, // игнорируем сигналы
        SYS_UMASK     => { let old = state.umask; state.umask = ebx & 0o777; old as i32 }
        SYS_TIMES     => 0,
        SYS_GETPRIORITY | SYS_SETPRIORITY => 0,
        SYS_SET_TID_ADDR | SYS_SET_ROBUST_LIST => 0,

        // ── Память ────────────────────────────────────────────────────────
        SYS_BRK => {
            if ebx == 0 || ebx < state.brk_start {
                state.brk as i32
            } else {
                state.brk = (ebx + 0xFFF) & 0xFFFFF000;
                state.brk as i32
            }
        }
        SYS_MMAP | SYS_MMAP2 => {
            // Упрощённо: выделяем из brk
            let len = (ecx + 0xFFF) & 0xFFFFF000;
            if len == 0 { return regs.eax = EINVAL as u32; }
            let addr = state.brk;
            state.brk += len;
            addr as i32
        }
        SYS_MUNMAP  => 0,
        SYS_MPROTECT => 0,

        // ── Файлы ─────────────────────────────────────────────────────────
        SYS_READ => sys_read(&mut state.fds, ebx as usize, ecx as *mut u8, edx as usize),
        SYS_WRITE => sys_write(&state.fds, ebx as usize, ecx as *const u8, edx as usize),
        SYS_OPEN  => sys_open(&mut state.fds, read_cstring(ebx), ecx, edx),
        SYS_CREAT => sys_open(&mut state.fds, read_cstring(ebx), 0o100 | 0o1, ecx),
        SYS_CLOSE => {
            let fd = ebx as usize;
            if fd < MAX_FDS { state.fds.fds[fd] = Fd::Closed; 0 } else { EBADF }
        }
        SYS_LSEEK => sys_lseek(&mut state.fds, ebx as usize, ecx as i32, edx),
        SYS_DUP   => sys_dup(&mut state.fds, ebx as usize, None),
        SYS_DUP2  => sys_dup(&mut state.fds, ebx as usize, Some(ecx as usize)),
        SYS_PIPE  => sys_pipe(&mut state.fds, ebx as *mut u32),
        SYS_FCNTL | SYS_FCNTL64 => sys_fcntl(&mut state.fds, ebx as usize, ecx, edx),
        SYS_FSYNC => 0,
        SYS_TRUNCATE  => sys_truncate(read_cstring(ebx), ecx as usize),
        SYS_FTRUNCATE => sys_ftruncate(&state.fds, ebx as usize, ecx as usize),
        SYS_FCHMOD | SYS_FCHOWN => 0,
        // POSIX chmod(path, mode) — у нас нет прав доступа, просто успех
        SYS_CHMOD  => 0,
        // POSIX chown(path, uid, gid) — у нас нет владельцев, просто успех
        SYS_CHOWN  => 0,
        // POSIX signal(signum, handler) — сохраняем адрес обработчика
        SYS_SIGNAL => {
            let signum = ebx as usize;
            let handler = ecx;
            if signum < 32 {
                let old = state.sighandlers[signum];
                state.sighandlers[signum] = handler;
                old as i32
            } else {
                EINVAL
            }
        }
        SYS_LINK      => sys_link(read_cstring(ebx), read_cstring(ecx)),
        SYS_SYMLINK   => sys_link(read_cstring(ebx), read_cstring(ecx)),
        SYS_UNLINK    => if fs::delete(read_cstring(ebx)) { 0 } else { ENOENT },
        SYS_RENAME    => if fs::rename(read_cstring(ebx), read_cstring(ecx)) { 0 } else { ENOENT },
        SYS_MKDIR     => if fs::mkdir(read_cstring(ebx)) { 0 } else { EEXIST },
        SYS_RMDIR     => if fs::delete(read_cstring(ebx)) { 0 } else { ENOENT },
        SYS_CHDIR     => { fs::set_cwd(read_cstring(ebx)); 0 }
        SYS_ACCESS | SYS_FACCESSAT => {
            let path = if nr == SYS_FACCESSAT { read_cstring(ecx) } else { read_cstring(ebx) };
            if fs::get(path).is_some() || path == "/" { 0 } else { ENOENT }
        }
        SYS_READLINK  => sys_readlink(read_cstring(ebx), ecx as *mut u8, edx as usize),

        // ── Директории ────────────────────────────────────────────────────
        SYS_GETDENTS | SYS_GETDENTS64 => {
            sys_getdents(&state.fds, ebx as usize, ecx as *mut u8, edx as usize, nr == SYS_GETDENTS64)
        }
        SYS_OPENAT  => {
            // AT_FDCWD = -100; игнорируем dirfd, используем путь как есть
            sys_open(&mut state.fds, read_cstring(ecx), edx, esi)
        }
        SYS_MKDIRAT => {
            if fs::mkdir(read_cstring(ecx)) { 0 } else { EEXIST }
        }
        SYS_UNLINKAT => {
            if fs::delete(read_cstring(ecx)) { 0 } else { ENOENT }
        }
        SYS_RENAMEAT => {
            if fs::rename(read_cstring(ecx), read_cstring(esi)) { 0 } else { ENOENT }
        }

        // ── Stat ──────────────────────────────────────────────────────────
        SYS_STAT | SYS_LSTAT => sys_stat(read_cstring(ebx), ecx as *mut u8, false),
        SYS_FSTAT            => sys_fstat(&state.fds, ebx as usize, ecx as *mut u8, false),
        SYS_STAT64 | SYS_LSTAT64 => sys_stat(read_cstring(ebx), ecx as *mut u8, true),
        SYS_FSTAT64          => sys_fstat(&state.fds, ebx as usize, ecx as *mut u8, true),

        // ── Система ───────────────────────────────────────────────────────
        SYS_UNAME => {
            let ptr = ecx as *mut u8;
            unsafe {
                write_cstring(ptr,         b"PINDOS");
                write_cstring(ptr.add(65), b"pindos");
                write_cstring(ptr.add(130),b"0.1.0");
                write_cstring(ptr.add(195),b"#1 SMP PINDOS");
                write_cstring(ptr.add(260),b"i686");
                write_cstring(ptr.add(325),b"(none)");
            }
            0
        }
        SYS_GETCWD => {
            let buf = ebx as *mut u8;
            let size = ecx as usize;
            let cwd = fs::cwd();
            let b = cwd.as_bytes();
            let len = b.len().min(size.saturating_sub(1));
            unsafe {
                for i in 0..len { *buf.add(i) = b[i]; }
                *buf.add(len) = 0;
            }
            ebx as i32
        }
        SYS_SETHOSTNAME => 0,
        SYS_SETRLIMIT | SYS_GETRLIMIT => {
            // Возвращаем "бесконечные" лимиты
            if nr == SYS_GETRLIMIT {
                let rl = ecx as *mut u32;
                unsafe { *rl = 0xFFFFFFFF; *rl.add(1) = 0xFFFFFFFF; }
            }
            0
        }
        SYS_GETRUSAGE => {
            let buf = ecx as *mut u8;
            unsafe { for i in 0..72usize { *buf.add(i) = 0; } }
            0
        }
        SYS_GETTIMEOFDAY => {
            let tv = ebx as *mut u32;
            unsafe { *tv = 0; *tv.add(1) = 0; }
            0
        }
        SYS_CLOCK_GETTIME => {
            let ts = ecx as *mut u32;
            unsafe { *ts = 0; *ts.add(1) = 0; }
            0
        }
        SYS_IOCTL => sys_ioctl(ebx as usize, ecx, edx),
        SYS_SIGACTION | SYS_SIGPROCMASK | SYS_SIGSUSPEND | SYS_SIGPENDING => {
            // Базовая поддержка сигналов — запоминаем обработчики
            if nr == SYS_SIGACTION && ebx < 32 {
                let act = ecx as *const u32;
                if !act.is_null() {
                    unsafe { state.sighandlers[ebx as usize] = *act; }
                }
            }
            0
        }

        _ => ENOSYS,
    };

    regs.eax = result as u32;
}


// ── Реализации syscall ─────────────────────────────────────────────────────

fn sys_read(fds: &mut FdTable, fd: usize, buf: *mut u8, len: usize) -> i32 {
    if fd >= MAX_FDS { return EBADF; }
    match &mut fds.fds[fd] {
        Fd::Stdin => {
            let mut n = 0;
            unsafe {
                while n < len {
                    let c = vga::read_char();
                    *buf.add(n) = c;
                    vga::put_char(c);
                    n += 1;
                    if c == b'\n' { break; }
                }
            }
            n as i32
        }
        Fd::File { name, name_len, offset, .. } => {
            let fname_bytes = &name[..*name_len];
            let fname = core::str::from_utf8(fname_bytes).unwrap_or("");
            if let Some(f) = fs::get(fname) {
                let content = f.content_str().as_bytes();
                let start = *offset;
                let n = (content.len().saturating_sub(start)).min(len);
                unsafe { for i in 0..n { *buf.add(i) = content[start + i]; } }
                *offset += n;
                n as i32
            } else { ENOENT }
        }
        _ => EBADF,
    }
}

fn sys_write(fds: &FdTable, fd: usize, buf: *const u8, len: usize) -> i32 {
    if fd >= MAX_FDS { return EBADF; }
    match fds.fds[fd] {
        Fd::Stdout | Fd::Stderr => {
            unsafe { for i in 0..len { vga::put_char(*buf.add(i)); } }
            len as i32
        }
        Fd::File { name, name_len, .. } => {
            let fname = core::str::from_utf8(&name[..name_len]).unwrap_or("");
            unsafe {
                let s = core::str::from_utf8(core::slice::from_raw_parts(buf, len)).unwrap_or("");
                fs::append(fname, s);
            }
            len as i32
        }
        _ => EBADF,
    }
}

fn sys_open(fds: &mut FdTable, path: &str, flags: u32, _mode: u32) -> i32 {
    const O_RDONLY: u32 = 0;
    const O_WRONLY: u32 = 1;
    const O_RDWR:   u32 = 2;
    const O_CREAT:  u32 = 0o100;
    const O_TRUNC:  u32 = 0o1000;
    const O_APPEND: u32 = 0o2000;
    const O_DIRECTORY: u32 = 0o200000;

    // Виртуальные пути /proc и /dev
    if path.starts_with("/proc/") || path.starts_with("/dev/") {
        return sys_open_virtual(fds, path, flags);
    }

    if flags & O_CREAT != 0 && fs::get(path).is_none() {
        fs::create(path, "");
    }
    if flags & O_TRUNC != 0 {
        fs::write(path, "");
    }

    match fs::get(path) {
        None => ENOENT,
        Some(e) => {
            if flags & O_DIRECTORY != 0 && !e.is_dir() { return ENOTDIR; }
            let fd = match fds.alloc() { Some(f) => f, None => return EMFILE };
            let mut name_arr = [0u8; 128];
            let b = path.as_bytes();
            let l = b.len().min(128);
            name_arr[..l].copy_from_slice(&b[..l]);
            let offset = if flags & O_APPEND != 0 { e.content_len } else { 0 };
            fds.fds[fd] = Fd::File { name: name_arr, name_len: l, offset, flags };
            fd as i32
        }
    }
}

fn sys_open_virtual(fds: &mut FdTable, path: &str, _flags: u32) -> i32 {
    // Создаём виртуальный файл с нужным содержимым
    let content: &str = match path {
        "/proc/self/maps"    => "00000000-bfffffff r-xp 00000000 00:00 0  [pindos]\n",
        "/proc/self/status"  => "Name: pindos\nPid: 1\nUid: 0 0 0 0\nGid: 0 0 0 0\n",
        "/proc/self/cmdline" => "pindos\0",
        "/proc/meminfo"      => "MemTotal: 32768 kB\nMemFree: 28672 kB\n",
        "/proc/cpuinfo"      => "processor: 0\nvendor_id: PINDOS\ncpu MHz: 100\n",
        "/dev/null"          => "",
        "/dev/zero"          => "",
        "/dev/urandom" | "/dev/random" => "\x42\x13\x37\x00",
        _ => return ENOENT,
    };

    // Временно создаём файл в FS
    let tmp_name = path; // используем путь как имя
    if fs::get(tmp_name).is_none() {
        fs::create(tmp_name, content);
    }

    let fd = match fds.alloc() { Some(f) => f, None => return EMFILE };
    let mut name_arr = [0u8; 128];
    let b = tmp_name.as_bytes();
    let l = b.len().min(128);
    name_arr[..l].copy_from_slice(&b[..l]);
    fds.fds[fd] = Fd::File { name: name_arr, name_len: l, offset: 0, flags: 0 };
    fd as i32
}

fn sys_lseek(fds: &mut FdTable, fd: usize, off: i32, whence: u32) -> i32 {
    if fd >= MAX_FDS { return EBADF; }
    if let Fd::File { ref mut offset, name, name_len, .. } = fds.fds[fd] {
        let new_off = match whence {
            0 => off as usize,
            1 => (*offset as i32 + off) as usize,
            2 => {
                let fname = core::str::from_utf8(&name[..name_len]).unwrap_or("");
                let size = fs::get(fname).map(|f| f.content_len).unwrap_or(0);
                (size as i32 + off) as usize
            }
            _ => return EINVAL,
        };
        *offset = new_off;
        new_off as i32
    } else { EBADF }
}

fn sys_dup(fds: &mut FdTable, old: usize, new: Option<usize>) -> i32 {
    if old >= MAX_FDS { return EBADF; }
    let entry = fds.fds[old];
    let new_fd = match new {
        Some(n) => { if n >= MAX_FDS { return EBADF; } n }
        None    => match fds.alloc() { Some(f) => f, None => return EMFILE },
    };
    fds.fds[new_fd] = entry;
    new_fd as i32
}

fn sys_pipe(fds: &mut FdTable, pipefd: *mut u32) -> i32 {
    let r = match fds.alloc() { Some(f) => f, None => return EMFILE };
    let w = match fds.alloc_from(r + 1) { Some(f) => f, None => return EMFILE };
    fds.fds[r] = Fd::PipeRead { buf: [0u8; 4096], len: 0, pos: 0 };
    fds.fds[w] = Fd::PipeWrite { peer: r };
    unsafe { *pipefd = r as u32; *pipefd.add(1) = w as u32; }
    0
}

fn sys_fcntl(fds: &mut FdTable, fd: usize, cmd: u32, arg: u32) -> i32 {
    const F_GETFD: u32 = 1;
    const F_SETFD: u32 = 2;
    const F_GETFL: u32 = 3;
    const F_SETFL: u32 = 4;
    const F_DUPFD: u32 = 0;
    if fd >= MAX_FDS { return EBADF; }
    match cmd {
        F_GETFD => 0,
        F_SETFD => 0,
        F_GETFL => {
            if let Fd::File { flags, .. } = fds.fds[fd] { flags as i32 } else { 0 }
        }
        F_SETFL => {
            if let Fd::File { ref mut flags, .. } = fds.fds[fd] { *flags = arg; }
            0
        }
        F_DUPFD => sys_dup(fds, fd, None),
        _ => EINVAL,
    }
}

fn sys_truncate(path: &str, size: usize) -> i32 {
    if let Some(f) = fs::get(path) {
        let content = f.content_str();
        let mut buf = [0u8; 4096];
        let n = content.len().min(size).min(4096);
        buf[..n].copy_from_slice(&content.as_bytes()[..n]);
        let s = core::str::from_utf8(&buf[..n]).unwrap_or("");
        fs::write(path, s);
        0
    } else { ENOENT }
}

fn sys_ftruncate(fds: &FdTable, fd: usize, size: usize) -> i32 {
    if fd >= MAX_FDS { return EBADF; }
    if let Fd::File { name, name_len, .. } = fds.fds[fd] {
        let fname = core::str::from_utf8(&name[..name_len]).unwrap_or("");
        sys_truncate(fname, size)
    } else { EBADF }
}

fn sys_link(src: &str, dst: &str) -> i32 {
    if fs::copy_file(src, dst) { 0 } else { ENOENT }
}

fn sys_readlink(path: &str, buf: *mut u8, size: usize) -> i32 {
    // Возвращаем путь как есть (нет символических ссылок)
    let b = path.as_bytes();
    let n = b.len().min(size);
    unsafe { for i in 0..n { *buf.add(i) = b[i]; } }
    n as i32
}

fn sys_ioctl(fd: usize, req: u32, arg: u32) -> i32 {
    match req {
        0x5401 => 0, // TCGETS
        0x5402 => 0, // TCSETS
        0x5413 => {  // TIOCGWINSZ
            let ws = arg as *mut u16;
            unsafe { *ws = 25; *ws.add(1) = 80; *ws.add(2) = 0; *ws.add(3) = 0; }
            0
        }
        0x5414 => 0, // TIOCSWINSZ
        0x540F => 0, // TIOCGPGRP
        0x5410 => 0, // TIOCSPGRP
        0x80045430 => 0, // TIOCGPTPEER
        _ => EINVAL,
    }
}

// ── stat структуры ────────────────────────────────────────────────────────

fn sys_stat(path: &str, buf: *mut u8, stat64: bool) -> i32 {
    match fs::get(path) {
        Some(e) => { fill_stat(buf, e.content_len, e.is_dir(), stat64); 0 }
        None if path == "/" => { fill_stat(buf, 0, true, stat64); 0 }
        None => ENOENT,
    }
}

fn sys_fstat(fds: &FdTable, fd: usize, buf: *mut u8, stat64: bool) -> i32 {
    if fd >= MAX_FDS { return EBADF; }
    match fds.fds[fd] {
        Fd::Stdin | Fd::Stdout | Fd::Stderr => {
            fill_stat(buf, 0, false, stat64); 0
        }
        Fd::File { name, name_len, .. } => {
            let fname = core::str::from_utf8(&name[..name_len]).unwrap_or("");
            sys_stat(fname, buf, stat64)
        }
        _ => EBADF,
    }
}

fn fill_stat(buf: *mut u8, size: usize, is_dir: bool, stat64: bool) {
    // Заполняем нулями
    let total = if stat64 { 96 } else { 88 };
    unsafe {
        for i in 0..total { *buf.add(i) = 0; }
        let buf32 = buf as *mut u32;
        // st_mode: S_IFREG=0o100000, S_IFDIR=0o040000
        let mode: u32 = if is_dir { 0o040755 } else { 0o100644 };
        if stat64 {
            // stat64: mode at offset 16
            *(buf32.add(4)) = mode;
            // st_size at offset 44 (u64)
            *(buf32.add(11)) = size as u32;
        } else {
            // stat: mode at offset 8
            *(buf32.add(2)) = mode;
            // st_size at offset 20
            *(buf32.add(5)) = size as u32;
        }
    }
}

// ── getdents ──────────────────────────────────────────────────────────────

fn sys_getdents(fds: &FdTable, fd: usize, buf: *mut u8, count: usize, is64: bool) -> i32 {
    if fd >= MAX_FDS { return EBADF; }
    let dir_path = if let Fd::File { name, name_len, .. } = fds.fds[fd] {
        let mut arr = [0u8; 128];
        arr[..name_len].copy_from_slice(&name[..name_len]);
        arr
    } else { return ENOTDIR; };

    let dir_str = core::str::from_utf8(&dir_path[..]).unwrap_or("/").trim_end_matches('\0');

    let mut pos = 0usize;
    for entry in fs::list_dir(dir_str) {
        let name = entry.name_str();
        let name_b = name.as_bytes();
        // dirent64: ino(8) + off(8) + reclen(2) + type(1) + name
        // dirent:   ino(4) + off(4) + reclen(2) + name + type(1)
        let rec_len = if is64 {
            ((8 + 8 + 2 + 1 + name_b.len() + 1 + 7) & !7)
        } else {
            ((4 + 4 + 2 + name_b.len() + 1 + 1 + 3) & !3)
        };
        if pos + rec_len > count { break; }
        unsafe {
            let p = buf.add(pos);
            if is64 {
                *(p as *mut u64) = 1; // ino
                *(p.add(8) as *mut u64) = (pos + rec_len) as u64; // off
                *(p.add(16) as *mut u16) = rec_len as u16;
                *p.add(18) = if entry.is_dir() { 4 } else { 8 }; // DT_DIR / DT_REG
                for (i, &b) in name_b.iter().enumerate() { *p.add(19 + i) = b; }
                *p.add(19 + name_b.len()) = 0;
            } else {
                *(p as *mut u32) = 1; // ino
                *(p.add(4) as *mut u32) = (pos + rec_len) as u32; // off
                *(p.add(8) as *mut u16) = rec_len as u16;
                for (i, &b) in name_b.iter().enumerate() { *p.add(10 + i) = b; }
                *p.add(10 + name_b.len()) = 0;
                *p.add(rec_len - 1) = if entry.is_dir() { 4 } else { 8 };
            }
        }
        pos += rec_len;
    }
    pos as i32
}

// ── Утилиты ───────────────────────────────────────────────────────────────

pub fn read_cstring(addr: u32) -> &'static str {
    if addr == 0 { return ""; }
    unsafe {
        let ptr = addr as *const u8;
        let mut len = 0;
        while *ptr.add(len) != 0 && len < 512 { len += 1; }
        core::str::from_utf8(core::slice::from_raw_parts(ptr, len)).unwrap_or("")
    }
}

unsafe fn write_cstring(dst: *mut u8, s: &[u8]) {
    for (i, &b) in s.iter().enumerate() { *dst.add(i) = b; }
    *dst.add(s.len()) = 0;
}
