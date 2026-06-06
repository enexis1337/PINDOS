//! PINDOS Service Manager (dealduck)
//!
//! Минимальная реализация для фазы 6:
//! 1. Загрузить systemd-like target файлы
//! 2. Запустить сервисы в правильном порядке
//! 3. Мониторить и перезапускать упавшие сервисы

mod manager;

use manager::ServiceManager;

fn main() {
    println!("[dealduck] PINDOS service manager starting...");

    // 1. Создать менеджер сервисов
    let mut manager = ServiceManager::new();

    // 2. Загрузить sysinit.target (обязательные системные сервисы)
    match manager.load_target("sysinit.target") {
        Ok(()) => println!("[dealduck] loaded sysinit.target"),
        Err(e) => {
            println!("[dealduck] ERROR: failed to load sysinit.target: {}", e);
            return;
        }
    }

    // 3. Загрузить multi-user.target (пользовательские сервисы)
    match manager.load_target("multi-user.target") {
        Ok(()) => println!("[dealduck] loaded multi-user.target"),
        Err(e) => {
            println!("[dealduck] WARNING: failed to load multi-user.target: {}", e);
        }
    }

    // 4. Запустить сервисы
    println!("[dealduck] starting services...");
    manager.run();
}
