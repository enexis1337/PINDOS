// System call interface for Hammam microkernel
#![no_std]

use crate::ipc::{sys_send, sys_recv, MessagePriority};
use crate::scheduler::{get_current_pid, get_current_tid};
use crate::memory::{FRAME_ALLOCATOR, PAGE_SIZE};

/// System call numbers
#[derive(Debug, Clone, Copy)]
#[repr(u64)]
pub enum SyscallNumber {
    Exit = 0,
    Fork = 1,
    Exec = 2,
    Wait = 3,
    Brk = 4,
    Mmap = 5,
    Munmap = 6,
    Send = 7,
    Recv = 8,
    Connect = 9,
    Bind = 10,
    Listen = 11,
    Accept = 12,
    GetPid = 20,
    GetTid = 21,
    Yield = 22,
    Sleep = 23,
    Open = 30,
    Close = 31,
    Read = 32,
    Write = 33,
    Lseek = 34,
    Stat = 35,
    Fstat = 36,
    Ioctl = 37,
    Dup = 38,
    Pipe = 39,
    Socket = 40,
    Shmget = 50,
    Shmat = 51,
    Shmctl = 52,
}

/// System call result
pub type SyscallResult = Result<usize, SyscallError>;

/// System call errors
#[derive(Debug, Clone, Copy)]
pub enum SyscallError {
    Success = 0,
    EpERM = 1,        // Operation not permitted
    EnoENT = 2,       // No such file or directory
    Esrch = 3,        // No such process
    EinTR = 4,        // Interrupted system call
    EIO = 5,          // I/O error
    Enxio = 6,        // No such device or address
    E2Big = 7,        // Argument list too long
    EnoExec = 8,      // Exec format error
    EbadF = 9,        // Bad file number
    EChild = 10,      // No child processes
    EAgain = 11,      // Try again
    EnoMem = 12,      // Out of memory
    EAcces = 13,      // Permission denied
    EBadAddr = 14,    // Bad address
    EnotBlk = 15,     // Block device required
    EBusy = 16,       // Device or resource busy
    EExist = 17,      // File exists
    ExDev = 18,       // Cross-device link
    EnoDev = 19,      // No such device
    EnotDir = 20,     // Not a directory
    EIsDir = 21,      // Is a directory
    Einval = 22,      // Invalid argument
    Enfile = 23,      // File table overflow
    Emfile = 24,      // Too many open files
    EnotTy = 25,      // Not a typewriter
    Efbig = 26,       // File too large
    Enospc = 27,      // No space left on device
    Espipe = 28,      // Illegal seek
    EROFS = 29,       // Read-only file system
    EMLink = 30,      // Too many links
    EPipe = 31,       // Broken pipe
    EDom = 32,        // Math argument out of domain
    ERange = 33,      // Math result not representable
}

/// Initialize system call interface
pub fn init() {
    // System calls are handled via interrupt 0x80 on x86_64
    // The syscall instruction is also available for faster calls
}

/// Handle a system call from user space
#[no_mangle]
pub extern "C" fn syscall_handler(
    syscall_num: u64,
    arg0: usize,
    arg1: usize,
    arg2: usize,
    arg3: usize,
    arg4: usize,
    arg5: usize,
) -> usize {
    let result = handle_syscall(
        SyscallNumber::try_from(syscall_num).unwrap_or(SyscallNumber::GetPid),
        arg0,
        arg1,
        arg2,
        arg3,
        arg4,
        arg5,
    );
    
    result.unwrap_or_else(|e| e as usize)
}

/// Dispatch system call
fn handle_syscall(
    num: SyscallNumber,
    arg0: usize,
    arg1: usize,
    arg2: usize,
    arg3: usize,
    arg4: usize,
    arg5: usize,
) -> SyscallResult {
    match num {
        // Process management
        SyscallNumber::Exit => sys_exit(arg0 as i32),
        SyscallNumber::Fork => sys_fork(),
        SyscallNumber::Exec => sys_exec(arg0 as *const u8),
        SyscallNumber::Wait => sys_wait(arg0 as *mut i32),
        SyscallNumber::Yield => sys_yield(),
        SyscallNumber::Sleep => sys_sleep(arg0),
        SyscallNumber::GetPid => Ok(get_current_pid()),
        SyscallNumber::GetTid => Ok(get_current_tid()),
        
        // Memory management
        SyscallNumber::Brk => sys_brk(arg0),
        SyscallNumber::Mmap => sys_mmap(arg0, arg1, arg2, arg3, arg4, arg5),
        SyscallNumber::Munmap => sys_munmap(arg0, arg1),
        
        // IPC
        SyscallNumber::Send => sys_ipc_send(arg0, arg1 as *const u8, arg2, arg3),
        SyscallNumber::Recv => sys_ipc_recv(arg0, arg1 as *mut u8, arg2, arg3),
        SyscallNumber::Connect => sys_connect(arg0, arg1),
        SyscallNumber::Bind => sys_bind(arg0, arg1),
        SyscallNumber::Listen => sys_listen(arg0, arg1),
        SyscallNumber::Accept => sys_accept(arg0),
        
        // File operations (redirected to VFS server)
        SyscallNumber::Open => sys_open(arg0 as *const u8, arg1, arg2),
        SyscallNumber::Close => sys_close(arg0),
        SyscallNumber::Read => sys_read(arg0, arg1 as *mut u8, arg2),
        SyscallNumber::Write => sys_write(arg0, arg1 as *const u8, arg2),
        SyscallNumber::Lseek => sys_lseek(arg0, arg1 as i64, arg2),
        SyscallNumber::Stat => sys_stat(arg0 as *const u8, arg1 as *mut Stat),
        SyscallNumber::Fstat => sys_fstat(arg0, arg1 as *mut Stat),
        SyscallNumber::Dup => sys_dup(arg0),
        SyscallNumber::Pipe => sys_pipe(arg0 as *mut i32),
        
        // Socket operations
        SyscallNumber::Socket => sys_socket(arg0, arg1, arg2),
        
        // Shared memory
        SyscallNumber::Shmget => sys_shmget(arg0, arg1, arg2),
        SyscallNumber::Shmat => sys_shmat(arg0, arg1, arg2),
        SyscallNumber::Shmctl => sys_shmctl(arg0, arg1, arg2),
    }
}

// System call implementations

fn sys_exit(code: i32) -> SyscallResult {
    // Terminate current process
    // In real implementation, cleanup resources and notify parent
    Err(SyscallError::Success)
}

fn sys_fork() -> SyscallResult {
    // Fork current process
    // In real implementation, duplicate address space and create new process
    Ok(0) // Child returns 0
}

fn sys_exec(path: *const u8) -> SyscallResult {
    // Replace current process image
    Err(SyscallError::Success)
}

fn sys_wait(status: *mut i32) -> SyscallResult {
    // Wait for child process
    Err(SyscallError::EChild)
}

fn sys_yield() -> SyscallResult {
    // Yield CPU to scheduler
    crate::scheduler::yield_cpu();
    Ok(0)
}

fn sys_sleep(millis: usize) -> SyscallResult {
    // Sleep for specified milliseconds
    crate::scheduler::sleep(millis / 10); // Assuming 10ms tick
    Ok(0)
}

fn sys_brk(addr: usize) -> SyscallResult {
    // Change data segment size
    Ok(0)
}

fn sys_mmap(addr: usize, len: usize, prot: usize, flags: usize, fd: usize, offset: usize) -> SyscallResult {
    // Map memory region
    // Redirects to VFS server for file-backed mappings
    let frame = FRAME_ALLOCATOR.lock().allocate_frames((len + PAGE_SIZE - 1) / PAGE_SIZE);
    Ok(frame.unwrap_or(0))
}

fn sys_munmap(addr: usize, len: usize) -> SyscallResult {
    // Unmap memory region
    let frame_addr = addr;
    FRAME_ALLOCATOR.lock().free_frame(frame_addr);
    Ok(0)
}

fn sys_ipc_send(to: usize, data: *const u8, len: usize, priority: usize) -> SyscallResult {
    let slice = unsafe { core::slice::from_raw_parts(data, len) };
    let prio = match priority {
        0 => MessagePriority::Low,
        1 => MessagePriority::Normal,
        2 => MessagePriority::High,
        3 => MessagePriority::Urgent,
        _ => MessagePriority::Normal,
    };
    sys_send(to, slice, prio)
}

fn sys_ipc_recv(from: usize, data: *mut u8, len: usize, blocking: usize) -> SyscallResult {
    let mut slice = unsafe { core::slice::from_raw_parts_mut(data, len) };
    sys_recv(from, &mut slice, blocking != 0)
}

fn sys_connect(endpoint: usize, _peer: usize) -> SyscallResult {
    // Connect to IPC endpoint
    Ok(0)
}

fn sys_bind(_endpoint: usize, _addr: usize) -> SyscallResult {
    Ok(0)
}

fn sys_listen(_endpoint: usize, _backlog: usize) -> SyscallResult {
    Ok(0)
}

fn sys_accept(_endpoint: usize) -> SyscallResult {
    Ok(0)
}

fn sys_open(path: *const u8, flags: usize, mode: usize) -> SyscallResult {
    // Redirect to VFS server via IPC
    let path_str = unsafe { core::ffi::CStr::from_ptr(path as *const i8) };
    let path_bytes = path_str.to_bytes();
    
    // Send open request to VFS server
    let mut response = [0u8; 8];
    sys_send(10, path_bytes, MessagePriority::Normal)?;
    sys_recv(11, &mut response, true)?;
    
    Ok(u64::from_le_bytes(response) as usize)
}

fn sys_close(fd: usize) -> SyscallResult {
    Ok(0)
}

fn sys_read(fd: usize, buf: *mut u8, count: usize) -> SyscallResult {
    let slice = unsafe { core::slice::from_raw_parts_mut(buf, count) };
    // Redirect to VFS server
    Ok(0)
}

fn sys_write(fd: usize, buf: *const u8, count: usize) -> SyscallResult {
    let slice = unsafe { core::slice::from_raw_parts(buf, count) };
    // Redirect to VFS server
    Ok(count)
}

fn sys_lseek(fd: usize, offset: i64, whence: usize) -> SyscallResult {
    Ok(0)
}

#[repr(C)]
pub struct Stat {
    st_dev: u64,
    st_ino: u64,
    st_mode: u32,
    st_nlink: u32,
    st_uid: u32,
    st_gid: u32,
    st_rdev: u64,
    st_size: u64,
    st_blksize: u64,
    st_blocks: u64,
    st_atime: u64,
    st_mtime: u64,
    st_ctime: u64,
}

fn sys_stat(path: *const u8, stat_buf: *mut Stat) -> SyscallResult {
    Err(SyscallError::EnoENT)
}

fn sys_fstat(fd: usize, stat_buf: *mut Stat) -> SyscallResult {
    Err(SyscallError::EbadF)
}

fn sys_dup(fd: usize) -> SyscallResult {
    Ok(fd)
}

fn sys_pipe(fds: *mut i32) -> SyscallResult {
    Ok(0)
}

fn sys_socket(domain: usize, socket_type: usize, protocol: usize) -> SyscallResult {
    // Redirect to network server
    Ok(0)
}

fn sys_shmget(key: usize, size: usize, flags: usize) -> SyscallResult {
    Ok(0)
}

fn sys_shmat(shmid: usize, shmaddr: usize, flags: usize) -> SyscallResult {
    Ok(0)
}

fn sys_shmctl(shmid: usize, cmd: usize, buf: usize) -> SyscallResult {
    Ok(0)
}

// TryFrom implementation for SyscallNumber
impl TryFrom<u64> for SyscallNumber {
    type Error = ();
    
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(SyscallNumber::Exit),
            1 => Ok(SyscallNumber::Fork),
            2 => Ok(SyscallNumber::Exec),
            3 => Ok(SyscallNumber::Wait),
            4 => Ok(SyscallNumber::Brk),
            5 => Ok(SyscallNumber::Mmap),
            6 => Ok(SyscallNumber::Munmap),
            7 => Ok(SyscallNumber::Send),
            8 => Ok(SyscallNumber::Recv),
            9 => Ok(SyscallNumber::Connect),
            10 => Ok(SyscallNumber::Bind),
            11 => Ok(SyscallNumber::Listen),
            12 => Ok(SyscallNumber::Accept),
            20 => Ok(SyscallNumber::GetPid),
            21 => Ok(SyscallNumber::GetTid),
            22 => Ok(SyscallNumber::Yield),
            23 => Ok(SyscallNumber::Sleep),
            30 => Ok(SyscallNumber::Open),
            31 => Ok(SyscallNumber::Close),
            32 => Ok(SyscallNumber::Read),
            33 => Ok(SyscallNumber::Write),
            34 => Ok(SyscallNumber::Lseek),
            35 => Ok(SyscallNumber::Stat),
            36 => Ok(SyscallNumber::Fstat),
            37 => Ok(SyscallNumber::Ioctl),
            38 => Ok(SyscallNumber::Dup),
            39 => Ok(SyscallNumber::Pipe),
            40 => Ok(SyscallNumber::Socket),
            50 => Ok(SyscallNumber::Shmget),
            51 => Ok(SyscallNumber::Shmat),
            52 => Ok(SyscallNumber::Shmctl),
            _ => Err(()),
        }
    }
}