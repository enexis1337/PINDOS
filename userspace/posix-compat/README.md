# POSIX Compatibility Layer for Hammam

Тонкая прослойка, которая транслирует POSIX вызовы в native Hammam syscalls. Позволяет портировать существующие Linux программы на PINDOS с минимальными изменениями.

## Архитектура

### Уровни

```
┌─────────────────────────────────────────┐
│  Linux/POSIX Программа (C/Rust)        │
├─────────────────────────────────────────┤
│  libc (glibc, musl) или posix-compat    │
├─────────────────────────────────────────┤
│  syscall wrapper (asm syscall)          │
├─────────────────────────────────────────┤
│  Hammam Kernel                          │
└─────────────────────────────────────────┘
```

### Поддерживаемые syscalls

#### I/O Operations
- `write(2)` → `sys_write` (rax=1)
- `read(2)` → `sys_read` (rax=0)
- `open(2)` → `sys_open` (TODO in Hammam)
- `close(2)` → `sys_close` (TODO in Hammam)

#### Process Control
- `exit(2)` → `sys_exit` (rax=60)
- `exit_group(2)` → `sys_exit`
- `fork(2)` — STUB (возвращает ENOSYS)
- `execve(2)` — STUB (возвращает ENOSYS)
- `waitpid(2)` — STUB

#### Memory Operations
- `memset(3)` — стандартная реализация
- `memcpy(3)` — стандартная реализация
- `memmove(3)` — стандартная реализация
- `malloc(3)` — STUB (требует глобального аллокатора)
- `free(3)` — STUB

#### String Operations
- `strlen(3)` — стандартная реализация
- `strcmp(3)` — стандартная реализация
- `strncmp(3)` — стандартная реализация
- `strcpy(3)` — стандартная реализация (UNSAFE)
- `strncpy(3)` — стандартная реализация

#### Utility Functions
- `puts(3)` — использует write(2)
- `strerror(3)` — описания ошибок
- `sleep(3)` — STUB
- `usleep(3)` — STUB
- `abort(3)` — вызывает exit(134)

## Использование

### Для Rust программ

```rust
extern crate posix_compat;

fn main() {
    // Использовать POSIX функции напрямую
    unsafe {
        let fd = posix_compat::open(b"/etc/hostname\0" as *const u8 as *const i8, 0);
        if fd >= 0 {
            // Использовать fd
            posix_compat::close(fd);
        }
    }
}
```

### Для C программ

```c
#include <unistd.h>
#include <string.h>

int main() {
    write(1, "Hello\n", 6);
    exit(0);
}
```

Скомпилировать с linker опциями:
```bash
gcc -c hello.c -o hello.o
ld -lposix_compat hello.o -o hello
```

## Errno Handling

Каждая POSIX функция устанавливает `errno` при ошибке:

```c
#include <errno.h>

int fd = open("/nonexistent", O_RDONLY);
if (fd < 0) {
    printf("Error: %s\n", strerror(errno));
}
```

## Расширение

Для добавления новых syscalls:

1. Добавить новый inline asm wrapper в `syscall_*()` функцию
2. Добавить POSIX wrapper функцию
3. Обновить errno handling

Пример:

```rust
#[inline]
unsafe fn syscall_mkdir(path: *const u8, mode: i32) -> isize {
    let ret: isize;
    core::arch::asm!(
        "syscall",
        in("rax") 83u64,  // mkdir syscall number
        in("rdi") path as u64,
        in("rsi") mode as u64,
        lateout("rax") ret,
        options(nostack)
    );
    ret
}

#[no_mangle]
pub unsafe extern "C" fn mkdir(path: *const c_char, mode: i32) -> c_int {
    let ret = syscall_mkdir(path as *const u8, mode);
    if ret < 0 {
        set_errno((-ret) as i32);
        -1
    } else {
        ret as c_int
    }
}
```

## Ограничения

- **no_std**: нет стандартной libC, только freestanding функции
- **No threading**: thread-local переменные не поддерживаются (используется глобальный ERRNO)
- **Limited process support**: fork/exec/waitpid не реализованы в Hammam
- **No allocator**: malloc/free требуют глобального аллокатора

## Ошибки и статусы

Все POSIX ошибки соответствуют стандартным кодам ошибок UNIX:

- `EPERM (1)` — Operation not permitted
- `ENOENT (2)` — No such file or directory
- `EBADF (9)` — Bad file descriptor
- `ENOMEM (12)` — Out of memory
- `ENOSYS (38)` — Function not implemented
- ...и другие

## Тестирование

```bash
cd userspace/posix-compat
cargo check
cargo test --lib
```

## Интеграция с net-server и dealduck

Когда сервисы в dealduck перейдут на использование POSIX API через эту прослойку, ядро сможет обеспечить полную поддержку без необходимости переписывать userspace код.
