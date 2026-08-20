#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {{
        extern crate alloc;
        let _fmt = core::format_args!($($arg)*);
        let _cap = 256usize;
        let _layout = core::alloc::Layout::array::<u8>(_cap).unwrap();
        let _ptr = unsafe { alloc::alloc::alloc(_layout) };
        if !_ptr.is_null() {
            struct HeapWriter(*mut u8, usize, usize);
            impl core::fmt::Write for HeapWriter {
                fn write_str(&mut self, s: &str) -> core::fmt::Result {
                    let b = s.as_bytes();
                    let remaining = self.2 - self.1;
                    if b.len() > remaining { return Err(core::fmt::Error); }
                    for i in 0..b.len() {
                        unsafe { core::ptr::write_volatile(self.0.add(self.1 + i), b[i]); }
                    }
                    self.1 += b.len();
                    Ok(())
                }
            }
            let mut _w = HeapWriter(_ptr, 0, _cap);
            let _ = core::fmt::Write::write_fmt(&mut _w, _fmt);
            if _w.1 < _cap {
                unsafe { core::ptr::write_volatile(_ptr.add(_w.1), b'\n'); }
                _w.1 += 1;
                #[allow(unused_unsafe)]
                unsafe {
                    core::arch::asm!(
                        "syscall",
                        in("rax") 1u64,
                        in("rdi") 1u64,
                        in("rsi") _ptr,
                        in("rdx") _w.1,
                    );
                }
            }
        }
    }};
}