// dealduckdaemon (dealduckd) — планировщик задач PINDOS
// Управляет фоновыми задачами, таймерами и системными процессами

const MAX_TASKS: usize = 16;

#[derive(Copy, Clone, PartialEq)]
pub enum TaskState {
    Inactive,
    Running,
    Waiting,
    Finished,
}

#[derive(Copy, Clone, PartialEq)]
pub enum TaskType {
    OneShot,    // Выполнить один раз
    Periodic,   // Повторять каждые N секунд
    Cron,       // По расписанию (упрощенный cron)
}

#[derive(Copy, Clone)]
pub struct Task {
    pub id:          u8,
    pub name:        [u8; 32],
    pub name_len:    usize,
    pub task_type:   TaskType,
    pub state:       TaskState,
    pub interval:    u32,        // Интервал в секундах
    pub next_run:    u32,        // Время следующего запуска (секунды с загрузки)
    pub command:     [u8; 128],  // Команда для выполнения
    pub cmd_len:     usize,
    pub run_count:   u32,        // Счетчик запусков
    pub last_result: i32,        // Код возврата последнего запуска
}

impl Task {
    pub const fn new() -> Self {
        Task {
            id: 0,
            name: [0u8; 32],
            name_len: 0,
            task_type: TaskType::OneShot,
            state: TaskState::Inactive,
            interval: 0,
            next_run: 0,
            command: [0u8; 128],
            cmd_len: 0,
            run_count: 0,
            last_result: 0,
        }
    }

    pub fn name_str(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("")
    }

    pub fn command_str(&self) -> &str {
        core::str::from_utf8(&self.command[..self.cmd_len]).unwrap_or("")
    }
}

pub struct DealduckDaemon {
    pub tasks:      [Task; MAX_TASKS],
    pub task_count: usize,
    pub next_id:    u8,
    pub uptime:     u32,  // Секунды с запуска системы
    pub enabled:    bool,
}

static mut DAEMON: DealduckDaemon = DealduckDaemon {
    tasks: [Task::new(); MAX_TASKS],
    task_count: 0,
    next_id: 1,
    uptime: 0,
    enabled: true,
};

impl DealduckDaemon {
    pub fn new() -> Self {
        DealduckDaemon {
            tasks: [Task::new(); MAX_TASKS],
            task_count: 0,
            next_id: 1,
            uptime: 0,
            enabled: true,
        }
    }

    /// Добавить новую задачу
    pub fn add_task(&mut self, name: &str, task_type: TaskType, interval: u32, command: &str) -> Option<u8> {
        if self.task_count >= MAX_TASKS {
            return None;
        }

        let id = self.next_id;
        self.next_id += 1;

        let task = &mut self.tasks[self.task_count];
        task.id = id;
        
        let name_bytes = name.as_bytes();
        let name_len = name_bytes.len().min(31);
        task.name[..name_len].copy_from_slice(&name_bytes[..name_len]);
        task.name_len = name_len;

        let cmd_bytes = command.as_bytes();
        let cmd_len = cmd_bytes.len().min(127);
        task.command[..cmd_len].copy_from_slice(&cmd_bytes[..cmd_len]);
        task.cmd_len = cmd_len;

        task.task_type = task_type;
        task.interval = interval;
        task.state = TaskState::Waiting;
        task.next_run = self.uptime + interval;
        task.run_count = 0;
        task.last_result = 0;

        self.task_count += 1;
        Some(id)
    }

    /// Удалить задачу по ID
    pub fn remove_task(&mut self, id: u8) -> bool {
        for i in 0..self.task_count {
            if self.tasks[i].id == id {
                // Сдвигаем остальные задачи
                for j in i..self.task_count - 1 {
                    self.tasks[j] = self.tasks[j + 1];
                }
                self.task_count -= 1;
                return true;
            }
        }
        false
    }

    /// Получить задачу по ID
    pub fn get_task(&self, id: u8) -> Option<&Task> {
        self.tasks[..self.task_count].iter().find(|t| t.id == id)
    }

    /// Получить задачу по ID (мутабельно)
    pub fn get_task_mut(&mut self, id: u8) -> Option<&mut Task> {
        self.tasks[..self.task_count].iter_mut().find(|t| t.id == id)
    }

    /// Основной цикл планировщика - вызывается периодически
    pub fn tick(&mut self) {
        if !self.enabled {
            return;
        }

        // Обновляем uptime (упрощенно - каждый тик = 1 секунда)
        self.uptime += 1;

        // Проверяем задачи на выполнение
        for i in 0..self.task_count {
            if self.tasks[i].state != TaskState::Waiting || self.uptime < self.tasks[i].next_run {
                continue;
            }

            self.tasks[i].state = TaskState::Running;

            // Копируем команду чтобы избежать borrow conflict
            let mut cmd_buf = [0u8; 128];
            let cmd_len = self.tasks[i].cmd_len;
            cmd_buf[..cmd_len].copy_from_slice(&self.tasks[i].command[..cmd_len]);
            let cmd = core::str::from_utf8(&cmd_buf[..cmd_len]).unwrap_or("");

            let result = Self::run_command(cmd);

            self.tasks[i].run_count += 1;
            self.tasks[i].last_result = result;

            match self.tasks[i].task_type {
                TaskType::OneShot => {
                    self.tasks[i].state = TaskState::Finished;
                }
                TaskType::Periodic | TaskType::Cron => {
                    let interval = self.tasks[i].interval;
                    self.tasks[i].next_run = self.uptime + interval;
                    self.tasks[i].state = TaskState::Waiting;
                }
            }
        }
    }

    /// Выполнить команду задачи
    fn run_command(command: &str) -> i32 {
        // Упрощенная реализация - выполняем через shell
        // В реальной системе здесь был бы fork/exec
        
        if command.is_empty() {
            return -1;
        }

        // Встроенные команды демона
        match command {
            "heartbeat" => {
                crate::vga::print_colored("[dealduckd] heartbeat\n", 0x08);
                0
            }
            "cleanup" => {
                crate::vga::print_colored("[dealduckd] cleanup\n", 0x08);
                // Здесь можно добавить очистку временных файлов
                0
            }
            "backup" => {
                crate::vga::print_colored("[dealduckd] backup\n", 0x08);
                // Здесь можно добавить резервное копирование
                0
            }
            _ => {
                // Выполняем через shell (если доступен)
                crate::vga::print_colored("[dealduckd] executing: ", 0x08);
                crate::vga::print(command);
                crate::vga::put_char(b'\n');
                
                // Пока возвращаем успех
                0
            }
        }
    }

    /// Получить статистику демона
    pub fn get_stats(&self) -> (usize, u32, bool) {
        let active_tasks = self.tasks[..self.task_count]
            .iter()
            .filter(|t| t.state != TaskState::Finished)
            .count();
        
        (active_tasks, self.uptime, self.enabled)
    }

    /// Включить/выключить демон
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if enabled {
            crate::vga::print_colored("[dealduckd] enabled\n", 0x0A);
        } else {
            crate::vga::print_colored("[dealduckd] disabled\n", 0x0C);
        }
    }

    /// Список всех задач
    pub fn list_tasks(&self) -> &[Task] {
        &self.tasks[..self.task_count]
    }
}

// ── Глобальный интерфейс ──────────────────────────────────────────────────

/// Инициализация демона
pub fn init() {
    unsafe {
        DAEMON = DealduckDaemon::new();
        
        // Добавляем несколько системных задач по умолчанию
        DAEMON.add_task("heartbeat", TaskType::Periodic, 60, "heartbeat");
        DAEMON.add_task("cleanup", TaskType::Periodic, 300, "cleanup");
    }
    
    crate::vga::print_colored("dealduckd: initialized\n", 0x0A);
}

/// Основной тик демона - вызывается из таймера
pub fn tick() {
    unsafe {
        DAEMON.tick();
    }
}

/// Добавить задачу
pub fn add_task(name: &str, task_type: TaskType, interval: u32, command: &str) -> Option<u8> {
    unsafe {
        DAEMON.add_task(name, task_type, interval, command)
    }
}

/// Удалить задачу
pub fn remove_task(id: u8) -> bool {
    unsafe {
        DAEMON.remove_task(id)
    }
}

/// Получить задачу
pub fn get_task(id: u8) -> Option<Task> {
    unsafe {
        DAEMON.get_task(id).copied()
    }
}

/// Список задач
pub fn list_tasks() -> &'static [Task] {
    unsafe {
        DAEMON.list_tasks()
    }
}

/// Статистика
pub fn get_stats() -> (usize, u32, bool) {
    unsafe {
        DAEMON.get_stats()
    }
}

/// Включить/выключить
pub fn set_enabled(enabled: bool) {
    unsafe {
        DAEMON.set_enabled(enabled);
    }
}

/// Команда для управления демоном из shell
pub fn dealduckd_command(args: &[&str]) {
    if args.is_empty() {
        print_help();
        return;
    }

    match args[0] {
        "status" => {
            let (active, uptime, enabled) = get_stats();
            crate::vga::print("dealduckd status:\n");
            crate::vga::print("  State: ");
            if enabled {
                crate::vga::print_colored("enabled", 0x0A);
            } else {
                crate::vga::print_colored("disabled", 0x0C);
            }
            crate::vga::put_char(b'\n');
            crate::vga::print("  Active tasks: ");
            print_number(active);
            crate::vga::put_char(b'\n');
            crate::vga::print("  Uptime: ");
            print_number(uptime as usize);
            crate::vga::print(" seconds\n");
        }
        
        "list" => {
            crate::vga::print("Scheduled tasks:\n");
            crate::vga::print("ID  Name           Type      Interval  State     Runs\n");
            crate::vga::print("--  ----           ----      --------  -----     ----\n");
            
            for task in list_tasks() {
                print_number(task.id as usize);
                crate::vga::print("   ");
                crate::vga::print(task.name_str());
                
                // Дополняем пробелами до 15 символов
                let name_len = task.name_str().len();
                for _ in name_len..15 {
                    crate::vga::put_char(b' ');
                }
                
                let type_str = match task.task_type {
                    TaskType::OneShot => "oneshot",
                    TaskType::Periodic => "periodic",
                    TaskType::Cron => "cron",
                };
                crate::vga::print(type_str);
                
                for _ in type_str.len()..10 {
                    crate::vga::put_char(b' ');
                }
                
                print_number(task.interval as usize);
                crate::vga::print("s       ");
                
                let state_str = match task.state {
                    TaskState::Inactive => "inactive",
                    TaskState::Running => "running",
                    TaskState::Waiting => "waiting",
                    TaskState::Finished => "finished",
                };
                crate::vga::print(state_str);
                
                for _ in state_str.len()..10 {
                    crate::vga::put_char(b' ');
                }
                
                print_number(task.run_count as usize);
                crate::vga::put_char(b'\n');
            }
        }
        
        "add" => {
            if args.len() < 4 {
                crate::vga::print("Usage: dealduckd add <name> <interval> <command>\n");
                return;
            }
            
            let name = args[1];
            let interval = parse_number(args[2]).unwrap_or(60) as u32;
            
            // Собираем команду из оставшихся аргументов
            let mut command_buf = [0u8; 128];
            let mut pos = 0;
            for i in 3..args.len() {
                if pos > 0 && pos < 127 {
                    command_buf[pos] = b' ';
                    pos += 1;
                }
                let arg_bytes = args[i].as_bytes();
                for &b in arg_bytes {
                    if pos < 127 {
                        command_buf[pos] = b;
                        pos += 1;
                    }
                }
            }
            let command = core::str::from_utf8(&command_buf[..pos]).unwrap_or("");
            
            if let Some(id) = add_task(name, TaskType::Periodic, interval, &command) {
                crate::vga::print("Task added with ID ");
                print_number(id as usize);
                crate::vga::put_char(b'\n');
            } else {
                crate::vga::print_colored("Failed to add task (queue full)\n", 0x0C);
            }
        }
        
        "remove" | "rm" => {
            if args.len() < 2 {
                crate::vga::print("Usage: dealduckd remove <id>\n");
                return;
            }
            
            if let Some(id) = parse_number(args[1]) {
                if remove_task(id as u8) {
                    crate::vga::print("Task removed\n");
                } else {
                    crate::vga::print_colored("Task not found\n", 0x0C);
                }
            } else {
                crate::vga::print_colored("Invalid task ID\n", 0x0C);
            }
        }
        
        "enable" => {
            set_enabled(true);
        }
        
        "disable" => {
            set_enabled(false);
        }
        
        "help" => {
            print_help();
        }
        
        _ => {
            crate::vga::print_colored("Unknown command. Use 'dealduckd help'\n", 0x0C);
        }
    }
}

fn print_help() {
    crate::vga::print("dealduckd - PINDOS task scheduler\n");
    crate::vga::print("Commands:\n");
    crate::vga::print("  status              - Show daemon status\n");
    crate::vga::print("  list                - List all tasks\n");
    crate::vga::print("  add <name> <sec> <cmd> - Add periodic task\n");
    crate::vga::print("  remove <id>         - Remove task by ID\n");
    crate::vga::print("  enable              - Enable daemon\n");
    crate::vga::print("  disable             - Disable daemon\n");
    crate::vga::print("  help                - Show this help\n");
}

fn print_number(n: usize) {
    if n == 0 {
        crate::vga::put_char(b'0');
        return;
    }
    
    let mut buf = [0u8; 20];
    let mut i = 0;
    let mut n = n;
    while n > 0 {
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        i += 1;
    }
    
    for j in (0..i).rev() {
        crate::vga::put_char(buf[j]);
    }
}

fn parse_number(s: &str) -> Option<usize> {
    let mut result = 0usize;
    for b in s.bytes() {
        if b >= b'0' && b <= b'9' {
            result = result * 10 + (b - b'0') as usize;
        } else {
            return None;
        }
    }
    Some(result)
}