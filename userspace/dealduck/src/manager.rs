extern crate alloc;

use alloc::vec::Vec;
use crate::unit::{ServiceUnit, ServiceState, RestartPolicy};

pub struct ServiceManager {
    units: Vec<ServiceUnit>,
}

impl ServiceManager {
    pub fn new() -> Self {
        Self { units: Vec::new() }
    }

    pub fn register(&mut self, name: &'static str, exec_path: &'static str) {
        self.units.push(ServiceUnit::new(name, exec_path));
    }

    pub fn start_all(&mut self) {
        for unit in &mut self.units {
            match Self::spawn(unit.exec_path) {
                Some(pid) => {
                    unit.state = ServiceState::Running;
                    unit.pid   = Some(pid);
                    crate::println!("[dealduck] started {} (pid={})", unit.name, pid);
                }
                None => {
                    unit.state = ServiceState::Failed;
                    crate::println!("[dealduck] FAILED to start {}", unit.name);
                }
            }
        }
    }

    /// Запустить процесс через syscall exec.
    /// Возвращает PID нового процесса или None при ошибке.
    fn spawn(path: &str) -> Option<u32> {
        let pid: i64;
        unsafe {
            core::arch::asm!(
                "syscall",
                in("rax") 2u64,              // sys_exec
                in("rdi") path.as_ptr() as u64,
                in("rsi") path.len() as u64,
                lateout("rax") pid,
            );
        }
        if pid < 0 { None } else { Some(pid as u32) }
    }

    pub fn run(&mut self) -> ! {
        loop {
            for unit in &mut self.units {
                if unit.state == ServiceState::Running {
                    if let Some(pid) = unit.pid {
                        let status = Self::waitpid_nonblock(pid);
                        if status == Some(0) || status.map(|s| s < 0).unwrap_or(false) {
                            unit.state = ServiceState::Failed;
                            crate::println!("[dealduck] {} exited, restarting...", unit.name);
                            if unit.restart == RestartPolicy::OnFailure {
                                if let Some(new_pid) = Self::spawn(unit.exec_path) {
                                    unit.state = ServiceState::Running;
                                    unit.pid   = Some(new_pid);
                                }
                            }
                        }
                    }
                }
            }

            unsafe { core::arch::asm!("hlt", options(nostack)) };
        }
    }

    fn waitpid_nonblock(pid: u32) -> Option<i32> {
        let result: i64;
        unsafe {
            core::arch::asm!(
                "syscall",
                in("rax") 3u64,    // sys_waitpid
                in("rdi") pid as u64,
                in("rsi") 1u64,    // WNOHANG
                lateout("rax") result,
            );
        }
        if result == 0 { None } else { Some(result as i32) }
    }
}
