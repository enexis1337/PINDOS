use crate::unit::{ServiceUnit, ServiceState, RestartPolicy};
use alloc::vec::Vec;

extern crate alloc;

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
        for i in 0..self.units.len() {
            let exec_path = self.units[i].exec_path;
            match self.spawn(exec_path) {
                Some(pid) => {
                    self.units[i].state = ServiceState::Running;
                    self.units[i].pid   = Some(pid);
                    crate::println!("[dealduck] started {} (pid={})", self.units[i].name, pid);
                }
                None => {
                    self.units[i].state = ServiceState::Failed;
                    crate::println!("[dealduck] FAILED to start {}", self.units[i].name);
                }
            }
        }
    }

    fn spawn(&self, path: &str) -> Option<u32> {
        let pid: i64;
        unsafe {
            core::arch::asm!(
                "syscall",
                in("rax") 2u64,
                in("rdi") path.as_ptr() as u64,
                in("rsi") path.len() as u64,
                lateout("rax") pid,
            );
        }
        if pid < 0 { None } else { Some(pid as u32) }
    }

    pub fn run(&mut self) -> ! {
        loop {
            for i in 0..self.units.len() {
                if self.units[i].state == ServiceState::Running {
                    if let Some(pid) = self.units[i].pid {
                        let status = self.waitpid_nonblock(pid);
                        if status == Some(0) || status.map(|s| s < 0).unwrap_or(false) {
                            self.units[i].state = ServiceState::Failed;
                            crate::println!("[dealduck] {} exited, restarting...", self.units[i].name);
                            if self.units[i].restart == RestartPolicy::OnFailure {
                                let exec_path = self.units[i].exec_path;
                                if let Some(new_pid) = self.spawn(exec_path) {
                                    self.units[i].state = ServiceState::Running;
                                    self.units[i].pid   = Some(new_pid);
                                }
                            }
                        }
                    }
                }
            }

            unsafe { core::arch::asm!("hlt", options(nostack)) };
        }
    }

    fn waitpid_nonblock(&self, pid: u32) -> Option<i32> {
        let result: i64;
        unsafe {
            core::arch::asm!(
                "syscall",
                in("rax") 3u64,
                in("rdi") pid as u64,
                in("rsi") 1u64,
                lateout("rax") result,
            );
        }
        if result == 0 { None } else { Some(result as i32) }
    }
}