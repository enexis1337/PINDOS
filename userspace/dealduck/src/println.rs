use core::fmt::Write;

pub struct SerialWriter;

impl Write for SerialWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let buf = s.as_bytes();
        let len = buf.len() as u64;
        let ptr = buf.as_ptr() as u64;
        let ret: i64;
        unsafe {
            core::arch::asm!(
                "syscall",
                in("rax") 1u64,
                in("rdi") 1u64,
                in("rsi") ptr,
                in("rdx") len,
                lateout("rax") ret,
            );
        }
        if ret < 0 {
            Err(core::fmt::Error)
        } else {
            Ok(())
        }
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        let _ = core::fmt::Write::write_fmt(
            &mut $crate::println::SerialWriter,
            core::format_args!($($arg)*),
        );
    }};
}

#[macro_export]
macro_rules! println {
    () => { $crate::print!("\n"); };
    ($($arg:tt)*) => { $crate::print!($($arg)*); $crate::print!("\n"); };
}
