//! Менеджер сервисов PINDOS

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Child};
use std::thread;
use std::time::Duration;

/// Состояние сервиса
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceState {
    #[allow(dead_code)]
    Starting,
    Running,
    Failed,
    Stopped,
}

/// Политика перезапуска
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartPolicy {
    No,
    OnFailure,
    Always,
}

/// Единица сервиса
#[derive(Debug, Clone)]
pub struct ServiceUnit {
    pub name: String,
    pub description: String,
    pub exec_start: String,
    pub restart: RestartPolicy,
    pub restart_sec: u64,
    pub after: Vec<String>,
    #[allow(dead_code)]
    pub wants: Vec<String>,
    pub state: ServiceState,
    pub pid: Option<u32>,
}

/// Менеджер сервисов
pub struct ServiceManager {
    units: BTreeMap<String, ServiceUnit>,
    running_processes: BTreeMap<String, Child>,
}

impl ServiceManager {
    /// Создать новый менеджер
    pub fn new() -> Self {
        ServiceManager {
            units: BTreeMap::new(),
            running_processes: BTreeMap::new(),
        }
    }

    /// Загрузить target и все его зависимости
    pub fn load_target(&mut self, target: &str) -> Result<(), String> {
        let target_path = PathBuf::from(format!("/etc/pindos/system/{}", target));

        // Если файл не существует, создать минимальный целевой файл
        if !target_path.exists() {
            println!(
                "[dealduck] target {} not found, using defaults",
                target_path.display()
            );
            self.load_default_target(target)?;
            return Ok(());
        }

        // Прочитать файл target
        let content = fs::read_to_string(&target_path)
            .map_err(|e| format!("Failed to read {}: {}", target_path.display(), e))?;

        // Парсить строки Wants=
        for line in content.lines() {
            if line.starts_with("Wants=") {
                let wants = &line[6..].trim();
                self.load_service(wants)?;
            }
        }

        Ok(())
    }

    /// Загрузить стандартные сервисы для target
    fn load_default_target(&mut self, target: &str) -> Result<(), String> {
        match target {
            "sysinit.target" => {
                // Системные инициализационные сервисы
                self.add_unit(ServiceUnit {
                    name: "vfs-mount.service".to_string(),
                    description: "Mount VFS filesystems".to_string(),
                    exec_start: "/usr/bin/vfs-mount".to_string(),
                    restart: RestartPolicy::No,
                    restart_sec: 0,
                    after: vec![],
                    wants: vec![],
                    state: ServiceState::Stopped,
                    pid: None,
                });
            }
            "multi-user.target" => {
                // Пользовательские сервисы
                self.add_unit(ServiceUnit {
                    name: "net-server.service".to_string(),
                    description: "PINDOS Network Stack".to_string(),
                    exec_start: "/usr/bin/net-server".to_string(),
                    restart: RestartPolicy::OnFailure,
                    restart_sec: 5,
                    after: vec!["sysinit.target".to_string()],
                    wants: vec![],
                    state: ServiceState::Stopped,
                    pid: None,
                });

                self.add_unit(ServiceUnit {
                    name: "nvme-driver.service".to_string(),
                    description: "NVMe Storage Driver".to_string(),
                    exec_start: "/usr/bin/nvme-driver".to_string(),
                    restart: RestartPolicy::OnFailure,
                    restart_sec: 5,
                    after: vec!["sysinit.target".to_string()],
                    wants: vec![],
                    state: ServiceState::Stopped,
                    pid: None,
                });
            }
            _ => {}
        }

        Ok(())
    }

    /// Загрузить сервис
    fn load_service(&mut self, service: &str) -> Result<(), String> {
        let service_path = PathBuf::from(format!("/etc/pindos/system/{}", service));

        if !service_path.exists() {
            return Err(format!("Service file not found: {}", service_path.display()));
        }

        let content = fs::read_to_string(&service_path)
            .map_err(|e| format!("Failed to read {}: {}", service_path.display(), e))?;

        let mut unit = ServiceUnit {
            name: service.to_string(),
            description: String::new(),
            exec_start: String::new(),
            restart: RestartPolicy::No,
            restart_sec: 0,
            after: vec![],
            wants: vec![],
            state: ServiceState::Stopped,
            pid: None,
        };

        // Парсить .service файл
        let mut in_service_section = false;
        for line in content.lines() {
            let line = line.trim();

            if line == "[Service]" {
                in_service_section = true;
                continue;
            }

            if line == "[Unit]" || line == "[Install]" {
                in_service_section = false;
            }

            if in_service_section {
                if line.starts_with("ExecStart=") {
                    unit.exec_start = line[10..].to_string();
                } else if line.starts_with("Restart=") {
                    let restart_policy = &line[8..];
                    unit.restart = match restart_policy {
                        "on-failure" => RestartPolicy::OnFailure,
                        "always" => RestartPolicy::Always,
                        _ => RestartPolicy::No,
                    };
                } else if line.starts_with("RestartSec=") {
                    let sec_str = &line[11..].trim_end_matches('s');
                    unit.restart_sec = sec_str.parse().unwrap_or(5);
                }
            } else if line.starts_with("Description=") {
                unit.description = line[12..].to_string();
            } else if line.starts_with("After=") {
                let after = &line[6..];
                unit.after.push(after.to_string());
            }
        }

        self.add_unit(unit);
        Ok(())
    }

    /// Добавить юнит в менеджер
    fn add_unit(&mut self, unit: ServiceUnit) {
        self.units.insert(unit.name.clone(), unit);
    }

    /// Запустить все сервисы
    pub fn run(&mut self) -> ! {
        // Сначала запустить сервисы без зависимостей (After=)
        let to_start: Vec<String> = self
            .units
            .values()
            .filter(|u| u.after.is_empty())
            .map(|u| u.name.clone())
            .collect();

        // Запустить первые сервисы
        for name in to_start {
            let unit = self.units.get_mut(&name);
            if let Some(unit) = unit {
                let exec_start = unit.exec_start.clone();
                let mut unit_clone = unit.clone();
                let _ = unit; // Отпустить ссылку перед вызовом start_unit
                unit_clone.exec_start = exec_start;
                self.start_unit(&mut unit_clone);
                if let Some(unit) = self.units.get_mut(&name) {
                    unit.state = unit_clone.state;
                    unit.pid = unit_clone.pid;
                }
            }
        }

        // Event loop
        loop {
            self.tick();
            thread::sleep(Duration::from_millis(100));
        }
    }

    /// Выполнить один цикл проверки
    fn tick(&mut self) {
        let names_to_check: Vec<String> = self
            .running_processes
            .keys()
            .cloned()
            .collect();

        let mut to_remove = vec![];
        let mut to_restart = vec![];

        for name in names_to_check {
            if let Some(child) = self.running_processes.get_mut(&name) {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        // Процесс завершился
                        to_remove.push(name.clone());

                        if let Some(unit) = self.units.get_mut(&name) {
                            if status.success() {
                                println!("[dealduck] {} exited successfully", name);
                                unit.state = ServiceState::Stopped;
                            } else {
                                println!(
                                    "[dealduck] {} failed with status {:?}",
                                    name,
                                    status.code()
                                );
                                unit.state = ServiceState::Failed;

                                // Перезапустить если необходимо
                                if unit.restart == RestartPolicy::OnFailure
                                    || unit.restart == RestartPolicy::Always
                                {
                                    to_restart.push((name.clone(), unit.restart_sec));
                                }
                            }
                        }
                    }
                    Ok(None) => {
                        // Процесс еще работает
                    }
                    Err(e) => {
                        println!("[dealduck] ERROR checking process {}: {}", name, e);
                    }
                }
            }
        }

        // Удалить завершившиеся процессы
        for name in to_remove {
            self.running_processes.remove(&name);
        }

        // Перезапустить упавшие сервисы
        for (name, restart_sec) in to_restart {
            if let Some(unit) = self.units.get(&name) {
                let mut unit_clone = unit.clone();
                let _ = unit; // Отпустить ссылку
                println!(
                    "[dealduck] restarting {} in {} seconds",
                    name, restart_sec
                );
                thread::sleep(Duration::from_secs(restart_sec));
                self.start_unit(&mut unit_clone);
                if let Some(unit) = self.units.get_mut(&name) {
                    unit.state = unit_clone.state;
                    unit.pid = unit_clone.pid;
                }
            }
        }
    }

    /// Запустить сервис
    fn start_unit(&mut self, unit: &mut ServiceUnit) {
        if unit.state == ServiceState::Running {
            return; // Уже работает
        }

        println!(
            "[dealduck] starting service {} ({})",
            unit.name, unit.description
        );

        // Парсить exec_start: "/path/to/binary --args"
        let mut parts = unit.exec_start.split_whitespace();
        let program = match parts.next() {
            Some(p) => p,
            None => {
                println!("[dealduck] ERROR: empty ExecStart for {}", unit.name);
                unit.state = ServiceState::Failed;
                return;
            }
        };

        let args: Vec<&str> = parts.collect();

        // Стартовать процесс
        match Command::new(program).args(&args).spawn() {
            Ok(child) => {
                let pid = child.id();
                println!("[dealduck] {} started with PID {}", unit.name, pid);
                unit.state = ServiceState::Running;
                unit.pid = Some(pid);
                self.running_processes.insert(unit.name.clone(), child);
            }
            Err(e) => {
                println!(
                    "[dealduck] ERROR: failed to start {}: {}",
                    unit.name, e
                );
                unit.state = ServiceState::Failed;
            }
        }
    }
}

impl Default for ServiceManager {
    fn default() -> Self {
        Self::new()
    }
}
