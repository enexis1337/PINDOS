// VESA/VBE framebuffer драйвер
// Инициализация через BIOS int 0x10 (до перехода в PM) или через Multiboot2 info
//
// Поддерживает: 32bpp, 24bpp, 16bpp (RGB565)
// Разрешения: 1024x768, 800x600, 640x480 (выбирается автоматически)

// Глобальный framebuffer
static mut FB: Framebuffer = Framebuffer::empty();
const BACKBUFFER_W: usize = 1024;
const BACKBUFFER_H: usize = 768;
static mut BACKBUFFER: [u32; BACKBUFFER_W * BACKBUFFER_H] = [0; BACKBUFFER_W * BACKBUFFER_H];
static mut BACKBUFFER_ENABLED: bool = false;

#[derive(Copy, Clone)]
pub struct Framebuffer {
    pub addr:   u32,   // физический адрес
    pub width:  u32,
    pub height: u32,
    pub pitch:  u32,   // байт на строку
    pub bpp:    u8,    // бит на пиксель (32, 24, 16)
    pub ready:  bool,
    pub active: bool,
}

impl Framebuffer {
    const fn empty() -> Self {
        Framebuffer { addr: 0, width: 0, height: 0, pitch: 0, bpp: 0, ready: false, active: false }
    }

    pub fn bytes_per_pixel(&self) -> u32 {
        (self.bpp as u32 + 7) / 8
    }
}

pub fn get() -> &'static Framebuffer {
    unsafe { &FB }
}

#[inline]
fn backbuffer_index(x: u32, y: u32) -> usize {
    y as usize * BACKBUFFER_W + x as usize
}

#[inline]
fn can_use_backbuffer(fb: &Framebuffer) -> bool {
    fb.bpp == 32 && (fb.width as usize) <= BACKBUFFER_W && (fb.height as usize) <= BACKBUFFER_H
}

#[inline]
fn put_pixel_hw(fb: &Framebuffer, x: u32, y: u32, color: u32) {
    let offset = y * fb.pitch + x * fb.bytes_per_pixel();
    let ptr = (fb.addr + offset) as *mut u8;

    unsafe {
        match fb.bpp {
            32 => {
                core::ptr::write_volatile(ptr as *mut u32, color);
            }
            24 => {
                core::ptr::write_volatile(ptr, (color & 0xFF) as u8);
                core::ptr::write_volatile(ptr.add(1), ((color >> 8) & 0xFF) as u8);
                core::ptr::write_volatile(ptr.add(2), ((color >> 16) & 0xFF) as u8);
            }
            16 => {
                let r = ((color >> 16) & 0xFF) >> 3;
                let g = ((color >> 8)  & 0xFF) >> 2;
                let b = (color & 0xFF) >> 3;
                let px = ((r << 11) | (g << 5) | b) as u16;
                core::ptr::write_volatile(ptr as *mut u16, px);
            }
            _ => {}
        }
    }
}

pub fn enable_backbuffer() {
    let fb = unsafe { &FB };
    if fb.ready && can_use_backbuffer(fb) {
        unsafe { BACKBUFFER_ENABLED = true; }
    }
}

pub fn disable_backbuffer() {
    unsafe { BACKBUFFER_ENABLED = false; }
}

pub fn backbuffer_enabled() -> bool {
    unsafe { BACKBUFFER_ENABLED }
}

pub fn present() {
    let fb = unsafe { &FB };
    if !fb.ready || !backbuffer_enabled() || !can_use_backbuffer(fb) {
        return;
    }

    for row in 0..fb.height {
        let src_row = row as usize * BACKBUFFER_W;
        let row_base = fb.addr + row * fb.pitch;
        for col in 0..fb.width {
            let color = unsafe { BACKBUFFER[src_row + col as usize] };
            let ptr = (row_base + col * 4) as *mut u32;
            unsafe {
                core::ptr::write_volatile(ptr, color);
            }
        }
    }
}

// ── Инициализация из Multiboot2 ───────────────────────────────────────────

/// Парсим Multiboot2 info и ищем framebuffer тег
pub fn init_from_multiboot2(mb2_info: u32) {
    if mb2_info == 0 { return; }

    let mut ptr = (mb2_info + 8) as *const u32; // пропускаем total_size и reserved
    let total_size = unsafe { *(mb2_info as *const u32) };
    let end = mb2_info + total_size;

    unsafe {
        while (ptr as u32) < end {
            let tag_type = *ptr;
            let tag_size = *ptr.add(1);

            match tag_type {
                // Тег 8 — framebuffer info
                8 => {
                    let fb_ptr = ptr as *const Mb2Framebuffer;
                    let fb = &*fb_ptr;
                    if fb.fb_type == 1 {
                        // Тип 1 = RGB direct color
                        FB = Framebuffer {
                            addr:   fb.addr_lo,
                            width:  fb.width,
                            height: fb.height,
                            pitch:  fb.pitch,
                            bpp:    fb.bpp,
                            ready:  true,
                            active: true,
                        };
                        return;
                    }
                }
                // Тег 0 — конец
                0 => break,
                _ => {}
            }

            // Следующий тег (выровнен на 8 байт)
            let next = (ptr as u32) + ((tag_size + 7) & !7);
            if next <= ptr as u32 { break; }
            ptr = next as *const u32;
        }
    }
}

#[repr(C, packed)]
struct Mb2Framebuffer {
    tag_type: u32,
    tag_size: u32,
    addr_lo:  u32,
    addr_hi:  u32,
    pitch:    u32,
    width:    u32,
    height:   u32,
    bpp:      u8,
    fb_type:  u8,
    _reserved: u16,
    // Далее идут поля цветовых масок — нам не нужны для RGB
}

// ── Инициализация через VESA BIOS (для чистого BIOS boot) ─────────────────
// Вызывается из bootloader ДО перехода в protected mode
// Результат сохраняется в специальной области памяти и читается ядром

pub const VESA_INFO_ADDR: u32 = 0x7E00; // сразу после MBR стека

#[repr(C, packed)]
pub struct VesaModeInfo {
    pub signature: u32,    // 0x56455341 'VESA'
    pub addr:      u32,
    pub width:     u16,
    pub height:    u16,
    pub pitch:     u16,
    pub bpp:       u8,
    pub _pad:      u8,
}

/// Читаем VESA info которую записал загрузчик
pub fn init_from_bios_info() {
    let info = unsafe { &*(VESA_INFO_ADDR as *const VesaModeInfo) };
    if info.signature != 0x56455341 { return; }

    unsafe {
        FB = Framebuffer {
            addr:   info.addr,
            width:  info.width as u32,
            height: info.height as u32,
            pitch:  info.pitch as u32,
            bpp:    info.bpp,
            ready:  true,
            active: true,
        };
    }
}

/// Парсим Multiboot1 info структуру (для QEMU -kernel с флагом 0x4)
pub fn init_from_multiboot1(mb1_info: u32) {
    if mb1_info == 0 { return; }

    #[repr(C, packed)]
    struct Mb1Info {
        flags:        u32,
        mem_lower:    u32,
        mem_upper:    u32,
        boot_device:  u32,
        cmdline:      u32,
        mods_count:   u32,
        mods_addr:    u32,
        syms:         [u32; 4],
        mmap_length:  u32,
        mmap_addr:    u32,
        drives_length: u32,
        drives_addr:  u32,
        config_table: u32,
        boot_loader:  u32,
        apm_table:    u32,
        vbe_control:  u32,
        vbe_mode_info: u32,
        vbe_mode:     u16,
        vbe_iface_seg: u16,
        vbe_iface_off: u16,
        vbe_iface_len: u16,
        fb_addr:      u64,
        fb_pitch:     u32,
        fb_width:     u32,
        fb_height:    u32,
        fb_bpp:       u8,
        fb_type:      u8,
    }

    let info = unsafe { &*(mb1_info as *const Mb1Info) };

    // Бит 12 = framebuffer info присутствует
    if info.flags & (1 << 12) == 0 { return; }
    // fb_type 1 = direct RGB color
    if info.fb_type != 1 { return; }

    unsafe {
        FB = Framebuffer {
            addr:   info.fb_addr as u32,
            width:  info.fb_width,
            height: info.fb_height,
            pitch:  info.fb_pitch,
            bpp:    info.fb_bpp,
            ready:  true,
            active: true,
        };
    }
}

/// Только определяем параметры Bochs VGA через PCI, не включаем LFB.
/// Безопасно вызывать до переключения из VGA текстового режима.
pub fn probe_only() {
    if let Some(bar0) = pci_read_bochs_lfb_addr() {
        unsafe {
            FB = Framebuffer {
                addr:   bar0,
                width:  1024,
                height: 768,
                pitch:  1024 * 4,
                bpp:    32,
                ready:  true,  // framebuffer найден, но режим ещё не активирован
                active: false,
            };
        }
    }
}
/// Включаем LFB режим — вызывать только перед переходом в графику.
pub fn enable_lfb() {
    let fb = unsafe { &FB };
    if !fb.ready { return; }

    const VBE_DISPI_INDEX_XRES:    u16 = 0x01;
    const VBE_DISPI_INDEX_YRES:    u16 = 0x02;
    const VBE_DISPI_INDEX_BPP:     u16 = 0x03;
    const VBE_DISPI_INDEX_ENABLE:  u16 = 0x04;
    const VBE_DISPI_INDEX_VIRT_WIDTH:  u16 = 0x06;
    const VBE_DISPI_INDEX_VIRT_HEIGHT: u16 = 0x07;
    const VBE_DISPI_INDEX_X_OFFSET:    u16 = 0x08;
    const VBE_DISPI_INDEX_Y_OFFSET:    u16 = 0x09;
    const VBE_DISPI_ENABLED:       u16 = 0x01;
    const VBE_DISPI_LFB_ENABLED:   u16 = 0x40;
    const VBE_DISPI_NOCLEARMEM:    u16 = 0x80;

    unsafe fn vbe_write(index: u16, value: u16) {
        core::arch::asm!("out dx, ax", in("dx") 0x01CEu16, in("ax") index);
        core::arch::asm!("out dx, ax", in("dx") 0x01CFu16, in("ax") value);
    }
    unsafe fn vbe_read(index: u16) -> u16 {
        let v: u16;
        core::arch::asm!("out dx, ax", in("dx") 0x01CEu16, in("ax") index);
        core::arch::asm!("in ax, dx", out("ax") v, in("dx") 0x01CFu16);
        v
    }

    let (w, h, bpp) = unsafe { (FB.width as u16, FB.height as u16, FB.bpp as u16) };

    unsafe {
        vbe_write(VBE_DISPI_INDEX_ENABLE, 0);
        vbe_write(VBE_DISPI_INDEX_XRES, w);
        vbe_write(VBE_DISPI_INDEX_YRES, h);
        vbe_write(VBE_DISPI_INDEX_BPP, bpp);
        vbe_write(VBE_DISPI_INDEX_VIRT_WIDTH, w);
        vbe_write(VBE_DISPI_INDEX_VIRT_HEIGHT, h);
        vbe_write(VBE_DISPI_INDEX_X_OFFSET, 0);
        vbe_write(VBE_DISPI_INDEX_Y_OFFSET, 0);
        vbe_write(VBE_DISPI_INDEX_ENABLE,
            VBE_DISPI_ENABLED | VBE_DISPI_LFB_ENABLED | VBE_DISPI_NOCLEARMEM);

        let real_w = vbe_read(VBE_DISPI_INDEX_XRES) as u32;
        let real_h = vbe_read(VBE_DISPI_INDEX_YRES) as u32;
        let real_bpp = vbe_read(VBE_DISPI_INDEX_BPP) as u8;
        let virt_w = vbe_read(VBE_DISPI_INDEX_VIRT_WIDTH) as u32;
        let bytes_pp = (real_bpp as u32 + 7) / 8;

        if real_w != 0 && real_h != 0 && real_bpp != 0 {
            FB.width = real_w;
            FB.height = real_h;
            FB.bpp = real_bpp;
            FB.pitch = virt_w.max(real_w) * bytes_pp;
            FB.active = true;
        }
    }
}

/// Программируем Bochs VGA через порты (QEMU -vga std).
pub fn try_qemu_vga_std() {
    const VBE_DISPI_INDEX_ID:      u16 = 0x00;
    const VBE_DISPI_INDEX_XRES:    u16 = 0x01;
    const VBE_DISPI_INDEX_YRES:    u16 = 0x02;
    const VBE_DISPI_INDEX_BPP:     u16 = 0x03;
    const VBE_DISPI_INDEX_ENABLE:  u16 = 0x04;
    const VBE_DISPI_INDEX_VIRT_WIDTH:  u16 = 0x06;
    const VBE_DISPI_INDEX_VIRT_HEIGHT: u16 = 0x07;
    const VBE_DISPI_INDEX_X_OFFSET:    u16 = 0x08;
    const VBE_DISPI_INDEX_Y_OFFSET:    u16 = 0x09;
    const VBE_DISPI_ENABLED:       u16 = 0x01;
    const VBE_DISPI_LFB_ENABLED:   u16 = 0x40;
    const VBE_DISPI_NOCLEARMEM:    u16 = 0x80;

    unsafe fn vbe_read(index: u16) -> u16 {
        let v: u16;
        core::arch::asm!("out dx, ax", in("dx") 0x01CEu16, in("ax") index);
        core::arch::asm!("in ax, dx",  out("ax") v, in("dx") 0x01CFu16);
        v
    }

    unsafe fn vbe_write(index: u16, value: u16) {
        core::arch::asm!("out dx, ax", in("dx") 0x01CEu16, in("ax") index);
        core::arch::asm!("out dx, ax", in("dx") 0x01CFu16, in("ax") value);
    }

    unsafe {
        // Проверяем ID — должен быть 0xB0C0..0xB0CF
        let id = vbe_read(VBE_DISPI_INDEX_ID);
        if id < 0xB0C0 || id > 0xB0CF {
            return; // Bochs VGA не найден
        }

        const W: u16 = 1024;
        const H: u16 = 768;
        const BPP: u16 = 32;

        vbe_write(VBE_DISPI_INDEX_ENABLE, 0);
        vbe_write(VBE_DISPI_INDEX_XRES, W);
        vbe_write(VBE_DISPI_INDEX_YRES, H);
        vbe_write(VBE_DISPI_INDEX_BPP, BPP);
        vbe_write(VBE_DISPI_INDEX_VIRT_WIDTH, W);
        vbe_write(VBE_DISPI_INDEX_VIRT_HEIGHT, H);
        vbe_write(VBE_DISPI_INDEX_X_OFFSET, 0);
        vbe_write(VBE_DISPI_INDEX_Y_OFFSET, 0);
        vbe_write(VBE_DISPI_INDEX_ENABLE,
            VBE_DISPI_ENABLED | VBE_DISPI_LFB_ENABLED | VBE_DISPI_NOCLEARMEM);

        let real_w = vbe_read(VBE_DISPI_INDEX_XRES) as u32;
        let real_h = vbe_read(VBE_DISPI_INDEX_YRES) as u32;
        let real_bpp = vbe_read(VBE_DISPI_INDEX_BPP) as u8;
        let virt_w = vbe_read(VBE_DISPI_INDEX_VIRT_WIDTH) as u32;
        let bytes_pp = (real_bpp as u32 + 7) / 8;

        let (bar0, _bar2, _cmd) = pci_enable_bochs_vga().unwrap_or((0, 0, 0));

        let fb_addr = if bar0 != 0 { bar0 } else { 0xE0000000 };

        FB = Framebuffer {
            addr:   fb_addr,
            width:  if real_w != 0 { real_w } else { W as u32 },
            height: if real_h != 0 { real_h } else { H as u32 },
            pitch:  if virt_w != 0 && bytes_pp != 0 { virt_w * bytes_pp } else { W as u32 * 4 },
            bpp:    if real_bpp != 0 { real_bpp } else { BPP as u8 },
            ready:  true,
            active: true,
        };
    }
}

fn pci_read_bochs_lfb_addr() -> Option<u32> {
    pci_read_bochs_bars().map(|(bar0, _, _)| bar0)
}

fn pci_read_bochs_bars() -> Option<(u32, u32, u16)> {
    unsafe fn pci_read32(bus: u8, dev: u8, func: u8, offset: u8) -> u32 {
        let addr: u32 = 0x80000000
            | ((bus as u32) << 16)
            | ((dev as u32) << 11)
            | ((func as u32) << 8)
            | (offset as u32 & 0xFC);
        core::arch::asm!("out dx, eax", in("dx") 0xCF8u16, in("eax") addr);
        let v: u32;
        core::arch::asm!("in eax, dx", out("eax") v, in("dx") 0xCFCu16);
        v
    }

    unsafe {
        for bus in 0u8..=255 {
            for dev in 0u8..32 {
                let id = pci_read32(bus, dev, 0, 0x00);
                if id == 0x11111234 { // Bochs VGA: vendor=0x1234, device=0x1111
                    let bar0 = pci_read32(bus, dev, 0, 0x10);
                    let bar2 = pci_read32(bus, dev, 0, 0x18);
                    let cmd = (pci_read32(bus, dev, 0, 0x04) & 0xFFFF) as u16;
                    return Some((bar0 & 0xFFFFFFF0, bar2 & 0xFFFFFFF0, cmd));
                }
            }
        }
    }
    None
}

fn pci_enable_bochs_vga() -> Option<(u32, u32, u16)> {
    unsafe fn pci_read32(bus: u8, dev: u8, func: u8, offset: u8) -> u32 {
        let addr: u32 = 0x80000000
            | ((bus as u32) << 16)
            | ((dev as u32) << 11)
            | ((func as u32) << 8)
            | (offset as u32 & 0xFC);
        core::arch::asm!("out dx, eax", in("dx") 0xCF8u16, in("eax") addr);
        let v: u32;
        core::arch::asm!("in eax, dx", out("eax") v, in("dx") 0xCFCu16);
        v
    }

    unsafe fn pci_write32(bus: u8, dev: u8, func: u8, offset: u8, value: u32) {
        let addr: u32 = 0x80000000
            | ((bus as u32) << 16)
            | ((dev as u32) << 11)
            | ((func as u32) << 8)
            | (offset as u32 & 0xFC);
        core::arch::asm!("out dx, eax", in("dx") 0xCF8u16, in("eax") addr);
        core::arch::asm!("out dx, eax", in("dx") 0xCFCu16, in("eax") value);
    }

    unsafe {
        for bus in 0u8..=255 {
            for dev in 0u8..32 {
                let id = pci_read32(bus, dev, 0, 0x00);
                if id == 0x11111234 {
                    let mut cmd_status = pci_read32(bus, dev, 0, 0x04);
                    cmd_status |= 0x00000003; // I/O + Memory Space Enable
                    pci_write32(bus, dev, 0, 0x04, cmd_status);

                    let bar0 = pci_read32(bus, dev, 0, 0x10) & 0xFFFFFFF0;
                    let bar2 = pci_read32(bus, dev, 0, 0x18) & 0xFFFFFFF0;
                    let cmd = (pci_read32(bus, dev, 0, 0x04) & 0xFFFF) as u16;
                    return Some((bar0, bar2, cmd));
                }
            }
        }
    }
    None
}

#[inline]
pub fn put_pixel(x: u32, y: u32, color: u32) {
    let fb = unsafe { &FB };
    if !fb.ready || x >= fb.width || y >= fb.height { return; }

    if backbuffer_enabled() && can_use_backbuffer(fb) {
        unsafe {
            BACKBUFFER[backbuffer_index(x, y)] = color;
        }
        return;
    }

    put_pixel_hw(fb, x, y, color);
}

pub fn fill_rect(x: u32, y: u32, w: u32, h: u32, color: u32) {
    let fb = unsafe { &FB };
    if !fb.ready || w == 0 || h == 0 || x >= fb.width || y >= fb.height {
        return;
    }

    let x_end = x.saturating_add(w).min(fb.width);
    let y_end = y.saturating_add(h).min(fb.height);

    for row in y..y_end {
        for col in x..x_end {
            put_pixel(col, row, color);
        }
    }
}

pub fn fill_rect_fast(x: u32, y: u32, w: u32, h: u32, color: u32) {
    let fb = unsafe { &FB };
    if !fb.ready || w == 0 || h == 0 || x >= fb.width || y >= fb.height { return; }

    let x_end = x.saturating_add(w).min(fb.width);
    let y_end = y.saturating_add(h).min(fb.height);
    let draw_w = x_end.saturating_sub(x);
    let draw_h = y_end.saturating_sub(y);
    if draw_w == 0 || draw_h == 0 { return; }

    if backbuffer_enabled() && can_use_backbuffer(fb) {
        for row in y..y_end {
            let row_start = backbuffer_index(x, row);
            let row_end = row_start + draw_w as usize;
            unsafe {
                BACKBUFFER[row_start..row_end].fill(color);
            }
        }
        return;
    }

    let bpp = fb.bytes_per_pixel();
    if bpp == 4 {
        // Для MMIO framebuffer безопаснее считать абсолютный адрес каждого пикселя,
        // чем ходить `.add()` по "не-Rust" памяти.
        for row in y..y_end {
            let row_base = fb.addr + row * fb.pitch + x * 4;
            for col in 0..draw_w {
                let ptr = (row_base + col * 4) as *mut u32;
                unsafe {
                    core::ptr::write_volatile(ptr, color);
                }
            }
        }
    } else {
        fill_rect(x, y, w, h, color);
    }
}

pub fn draw_hline(x: u32, y: u32, w: u32, color: u32) {
    fill_rect_fast(x, y, w, 1, color);
}

pub fn draw_vline(x: u32, y: u32, h: u32, color: u32) {
    let fb = unsafe { &FB };
    if !fb.ready || h == 0 || x >= fb.width || y >= fb.height { return; }

    let y_end = y.saturating_add(h).min(fb.height);
    for row in y..y_end { put_pixel(x, row, color); }
}

pub fn draw_rect_outline(x: u32, y: u32, w: u32, h: u32, color: u32) {
    if w == 0 || h == 0 { return; }
    draw_hline(x, y, w, color);
    draw_hline(x, y + h - 1, w, color);
    draw_vline(x, y, h, color);
    draw_vline(x + w - 1, y, h, color);
}

/// Очистить экран
pub fn clear(color: u32) {
    let fb = unsafe { &FB };
    if !fb.ready { return; }
    fill_rect_fast(0, 0, fb.width, fb.height, color);
}

// ── Шрифт 8x16 (встроенный) ───────────────────────────────────────────────

// Оригинальный шрифт — оставлен для совместимости
static FONT_8X16_OLD: &[u8] = include_bytes!("font8x16.bin");
// Новый шрифт — более чистые глифы
static FONT_8X16_NEW: &[u8] = include_bytes!("font8x16_new.bin");

pub const FONT_W: u32 = 8;
pub const FONT_H: u32 = 16;

// Активный шрифт: false = старый, true = новый
static mut USE_NEW_FONT: bool = false;

pub fn set_font_new(enable: bool) {
    unsafe { USE_NEW_FONT = enable; }
}

pub fn is_new_font() -> bool {
    unsafe { USE_NEW_FONT }
}

#[inline]
fn active_font() -> &'static [u8] {
    if unsafe { USE_NEW_FONT } { FONT_8X16_NEW } else { FONT_8X16_OLD }
}

pub fn draw_char(x: u32, y: u32, c: u8, fg: u32, bg: u32) {
    let ch = if c >= 32 && c < 128 { c - 32 } else { 0 };
    let font = active_font();
    let glyph_off = ch as usize * FONT_H as usize;

    if glyph_off + FONT_H as usize > font.len() { return; }

    for row in 0..FONT_H {
        let byte = font[glyph_off + row as usize];
        for col in 0..FONT_W {
            let pixel = (byte >> (7 - col)) & 1;
            if pixel != 0 {
                put_pixel(x + col, y + row, fg);
            } else if bg != 0xFF000000 {
                put_pixel(x + col, y + row, bg);
            }
        }
    }
}

pub fn draw_str(x: u32, y: u32, s: &str, fg: u32, bg: u32) {
    let mut cx = x;
    for b in s.bytes() {
        if b == b'\n' { return; }
        draw_char(cx, y, b, fg, bg);
        cx += FONT_W;
        if cx + FONT_W > unsafe { FB.width } { break; }
    }
}

// ── Цвета (0xRRGGBB) ──────────────────────────────────────────────────────

pub const BLACK:   u32 = 0x000000;
pub const WHITE:   u32 = 0xFFFFFF;
pub const RED:     u32 = 0xFF0000;
pub const GREEN:   u32 = 0x00FF00;
pub const BLUE:    u32 = 0x0000FF;
pub const CYAN:    u32 = 0x00FFFF;
pub const YELLOW:  u32 = 0xFFFF00;
pub const MAGENTA: u32 = 0xFF00FF;
pub const LGRAY:   u32 = 0xC0C0C0;
pub const DGRAY:   u32 = 0x404040;
pub const MGRAY:   u32 = 0x808080;

// Mell цвета
pub const MELL_DESKTOP_BG: u32 = 0x4A7C59; // тёмно-зелёный
pub const MELL_TITLEBAR:   u32 = 0x5A5A5A;
pub const MELL_TITLEBAR_ACTIVE: u32 = 0x3A6EA5;
pub const MELL_WINDOW_BG:  u32 = 0xF0F0F0;
pub const MELL_TASKBAR:    u32 = 0xD4D0C8;
pub const MELL_TOPBAR:     u32 = 0x2B2B2B;
pub const MELL_TEXT:       u32 = 0x000000;
pub const MELL_TEXT_LIGHT: u32 = 0xFFFFFF;
pub const MELL_BORDER:     u32 = 0x808080;
pub const MELL_BTN_CLOSE:  u32 = 0xFF5F57;
pub const MELL_BTN_MIN:    u32 = 0xFFBD2E;
pub const MELL_BTN_MAX:    u32 = 0x28C840;

// ── Рисование ─────────────────────────────────────────────────────────────
