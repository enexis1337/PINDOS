/// Сохранённый контекст задачи: callee-saved регистры x86_64.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Context {
    pub rbx: u64,
    pub rbp: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rsp: u64,
}

impl Context {
    pub const fn new() -> Self {
        Self {
            rbx: 0,
            rbp: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            rsp: 0,
        }
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

/// Переключает выполнение между двумя контекстами задач.
///
/// # Safety
///
/// - `from` и `to` должны быть валидными указателями на инициализированные `Context`.
/// - `from` обязан оставаться доступным после переключения, так как текущий стек будет сохранён в нём.
/// - `to.rsp` должен указывать на корректный стек, содержащий валидный адрес возврата.
/// - Функция должна вызываться только на архитектуре x86_64 с соответствующей ABI.
pub unsafe extern "C" fn switch_context(from: *mut Context, to: *const Context) {
    unsafe {
        core::arch::asm!(
            "mov [rdi + 0], rbx",
            "mov [rdi + 8], rbp",
            "mov [rdi + 16], r12",
            "mov [rdi + 24], r13",
            "mov [rdi + 32], r14",
            "mov [rdi + 40], r15",
            "mov [rdi + 48], rsp",
            "mov rsp, [rsi + 48]",
            "mov rbx, [rsi + 0]",
            "mov rbp, [rsi + 8]",
            "mov r12, [rsi + 16]",
            "mov r13, [rsi + 24]",
            "mov r14, [rsi + 32]",
            "mov r15, [rsi + 40]",
            "ret",
            in("rdi") from,
            in("rsi") to,
            options(noreturn)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kprintln;

    static mut MAIN_CONTEXT: Context = Context::new();
    static mut CHILD_CONTEXT: Context = Context::new();
    static mut CHILD_STACK: [u8; 256] = [0; 256];

    extern "C" fn child_entry() -> ! {
        unsafe {
            kprintln!("[context test] child context entered");
            switch_context(&mut CHILD_CONTEXT as *mut Context, &MAIN_CONTEXT as *const Context);
        }
        loop {}
    }

    #[test]
    fn switch_context_via_com1() {
        unsafe {
            let stack_top = CHILD_STACK.as_mut_ptr().add(CHILD_STACK.len());
            let aligned_top = (stack_top as usize & !0x0F) as *mut u8;
            let return_slot = aligned_top as *mut u64;
            *return_slot = child_entry as u64;

            CHILD_CONTEXT.rsp = return_slot as u64;
            kprintln!("[context test] switching to child context");
            switch_context(&mut MAIN_CONTEXT as *mut Context, &CHILD_CONTEXT as *const Context);
            kprintln!("[context test] returned to main context");
        }
    }
}
