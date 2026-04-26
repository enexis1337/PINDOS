// PNG viewer — конвертация PNG в ASCII-арт для VGA 80x25
//
// Парсим PNG минимально: только IHDR + IDAT (deflate не реализуем полностью,
// используем упрощённый парсер для несжатых/слабосжатых PNG)
// Для полноценного PNG нужен zlib/deflate — добавим базовую реализацию

use crate::mell::vga_gui::*;

// PNG сигнатура
const PNG_SIG: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];

// ASCII символы по яркости (от тёмного к светлому)
const ASCII_RAMP: &[u8] = b" .:-=+*#%@";

pub enum PngError {
    TooSmall,
    BadSignature,
    NoIhdr,
    Unsupported,
    DecompressError,
}

pub struct PngInfo {
    pub width:  u32,
    pub height: u32,
    pub depth:  u8,
    pub color:  u8, // 0=gray, 2=rgb, 3=indexed, 4=gray+alpha, 6=rgba
}

/// Отображает PNG как ASCII-арт в окне
pub fn render_png_ascii(data: &[u8], win: &Window) -> Result<(), PngError> {
    if data.len() < 8 { return Err(PngError::TooSmall); }
    if data[..8] != PNG_SIG { return Err(PngError::BadSignature); }

    let info = parse_ihdr(data)?;

    let ix = win.inner_x();
    let iy = win.inner_y();
    let iw = win.inner_w();
    let ih = win.inner_h();

    // Показываем информацию о файле
    fill_rect(ix, iy, iw, ih, b' ', MELL_WINDOW);
    put_str_at(ix, iy, "PNG Image", CYAN);

    // Размеры
    let mut info_buf = [0u8; 40];
    let mut pos = 0;
    write_u32(&mut info_buf, &mut pos, info.width);
    info_buf[pos] = b'x'; pos += 1;
    write_u32(&mut info_buf, &mut pos, info.height);
    info_buf[pos] = b' '; pos += 1;
    let color_name = match info.color {
        0 => b"Gray" as &[u8],
        2 => b"RGB",
        3 => b"Indexed",
        4 => b"Gray+A",
        6 => b"RGBA",
        _ => b"Unknown",
    };
    for &b in color_name { if pos < 40 { info_buf[pos] = b; pos += 1; } }
    put_str_at(ix + 10, iy, core::str::from_utf8(&info_buf[..pos]).unwrap_or(""), BLACK);

    // Пытаемся декодировать и отобразить
    match decode_and_render(data, &info, ix, iy + 1, iw, ih - 2) {
        Ok(_) => {}
        Err(_) => {
            // Если декодирование не удалось — показываем заглушку
            render_placeholder(&info, ix, iy + 1, iw, ih - 2);
        }
    }

    Ok(())
}

fn parse_ihdr(data: &[u8]) -> Result<PngInfo, PngError> {
    // Первый чанк после сигнатуры — IHDR
    let mut pos = 8;
    if pos + 12 > data.len() { return Err(PngError::NoIhdr); }

    let chunk_len = u32::from_be_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
    pos += 4;

    if &data[pos..pos+4] != b"IHDR" { return Err(PngError::NoIhdr); }
    pos += 4;

    if chunk_len < 13 || pos + 13 > data.len() { return Err(PngError::NoIhdr); }

    let width  = u32::from_be_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]);
    let height = u32::from_be_bytes([data[pos+4], data[pos+5], data[pos+6], data[pos+7]]);
    let depth  = data[pos+8];
    let color  = data[pos+9];

    Ok(PngInfo { width, height, depth, color })
}

fn decode_and_render(data: &[u8], info: &PngInfo, x: usize, y: usize, w: usize, h: usize) -> Result<(), PngError> {
    // Собираем все IDAT чанки
    let mut idat_buf = [0u8; 65536];
    let mut idat_len = 0usize;
    let mut pos = 8;

    while pos + 8 <= data.len() {
        let chunk_len = u32::from_be_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
        pos += 4;
        if pos + 4 > data.len() { break; }
        let chunk_type = &data[pos..pos+4];
        pos += 4;

        if chunk_type == b"IDAT" {
            let copy = chunk_len.min(idat_buf.len() - idat_len);
            if pos + copy <= data.len() {
                idat_buf[idat_len..idat_len+copy].copy_from_slice(&data[pos..pos+copy]);
                idat_len += copy;
            }
        }
        pos += chunk_len + 4; // данные + CRC
    }

    if idat_len == 0 { return Err(PngError::DecompressError); }

    // Декомпрессия zlib (упрощённая — только store блоки)
    let mut raw = [0u8; 32768];
    let raw_len = zlib_decompress(&idat_buf[..idat_len], &mut raw)?;

    // Рендерим в ASCII
    render_raw_to_ascii(info, &raw[..raw_len], x, y, w, h);
    Ok(())
}

/// Упрощённый zlib декомпрессор — поддерживает только DEFLATE store блоки (тип 00)
/// Для реальных PNG нужен полный DEFLATE, но многие простые PNG используют store
fn zlib_decompress(input: &[u8], output: &mut [u8]) -> Result<usize, PngError> {
    if input.len() < 2 { return Err(PngError::DecompressError); }

    // Пропускаем zlib заголовок (2 байта)
    let mut pos = 2;
    let mut out_pos = 0;

    while pos < input.len() {
        if pos >= input.len() { break; }
        let bfinal = input[pos] & 1;
        let btype  = (input[pos] >> 1) & 3;
        pos += 1;

        match btype {
            0 => {
                // Store блок — без сжатия
                // Выравниваем на байт (пропускаем остаток текущего байта)
                if pos + 4 > input.len() { return Err(PngError::DecompressError); }
                let len  = u16::from_le_bytes([input[pos], input[pos+1]]) as usize;
                let _nlen = u16::from_le_bytes([input[pos+2], input[pos+3]]);
                pos += 4;
                if pos + len > input.len() { return Err(PngError::DecompressError); }
                let copy = len.min(output.len() - out_pos);
                output[out_pos..out_pos+copy].copy_from_slice(&input[pos..pos+copy]);
                out_pos += copy;
                pos += len;
            }
            1 | 2 => {
                // Fixed/Dynamic Huffman — базовая реализация
                // Для простоты возвращаем ошибку, рендерим заглушку
                return Err(PngError::DecompressError);
            }
            _ => return Err(PngError::DecompressError),
        }

        if bfinal == 1 { break; }
    }

    Ok(out_pos)
}

fn render_raw_to_ascii(info: &PngInfo, raw: &[u8], x: usize, y: usize, w: usize, h: usize) {
    let img_w = info.width as usize;
    let img_h = info.height as usize;
    if img_w == 0 || img_h == 0 { return; }

    let bytes_per_pixel: usize = match info.color {
        0 => 1, // gray
        2 => 3, // rgb
        4 => 2, // gray+alpha
        6 => 4, // rgba
        _ => 3,
    };
    let stride = 1 + img_w * bytes_per_pixel; // +1 для filter byte

    for row in 0..h {
        let img_row = row * img_h / h;
        let row_start = img_row * stride;
        if row_start + stride > raw.len() { break; }

        // filter byte пропускаем
        let row_data = &raw[row_start + 1..row_start + stride];

        for col in 0..w {
            let img_col = col * img_w / w;
            let pixel_off = img_col * bytes_per_pixel;
            if pixel_off >= row_data.len() { break; }

            // Вычисляем яркость
            let brightness: u8 = match info.color {
                0 => row_data[pixel_off],
                2 => {
                    let r = row_data[pixel_off] as u16;
                    let g = row_data.get(pixel_off+1).copied().unwrap_or(0) as u16;
                    let b = row_data.get(pixel_off+2).copied().unwrap_or(0) as u16;
                    ((r * 299 + g * 587 + b * 114) / 1000) as u8
                }
                4 => row_data[pixel_off],
                6 => {
                    let r = row_data[pixel_off] as u16;
                    let g = row_data.get(pixel_off+1).copied().unwrap_or(0) as u16;
                    let b = row_data.get(pixel_off+2).copied().unwrap_or(0) as u16;
                    ((r * 299 + g * 587 + b * 114) / 1000) as u8
                }
                _ => 128,
            };

            // Яркость → ASCII символ
            let idx = (brightness as usize * (ASCII_RAMP.len() - 1)) / 255;
            let ch = ASCII_RAMP[idx];

            // Яркость → цвет
            let attr = brightness_to_color(brightness);
            put_char_at(x + col, y + row, ch, attr);
        }
    }
}

fn brightness_to_color(b: u8) -> u32 {
    match b {
        0..=51   => 0x404040,
        52..=102 => 0x808080,
        103..=153 => 0xAAAAAA,
        154..=204 => 0xCCCCCC,
        205..=255 => 0xFFFFFF,
    }
}

fn render_placeholder(info: &PngInfo, x: usize, y: usize, w: usize, h: usize) {
    use crate::mell::vga_gui::{LGRAY, DGRAY, CYAN, BLACK};
    fill_rect(x, y, w, h, b' ', LGRAY);
    draw_hline(x, y, w, DGRAY);
    draw_hline(x, y + h - 1, w, DGRAY);
    draw_vline(x, y, h, DGRAY);
    draw_vline(x + w - 1, y, h, DGRAY);

    let mid_y = y + h / 2;
    put_str_at(x + 2, mid_y - 1, "[PNG Image]", CYAN);

    let mut buf = [0u8; 32];
    let mut pos = 0;
    write_u32(&mut buf, &mut pos, info.width);
    buf[pos] = b'x'; pos += 1;
    write_u32(&mut buf, &mut pos, info.height);
    put_str_at(x + 2, mid_y, core::str::from_utf8(&buf[..pos]).unwrap_or(""), BLACK);
    put_str_at(x + 2, mid_y + 1, "(Compressed PNG - use store mode)", DGRAY);
}

fn write_u32(buf: &mut [u8], pos: &mut usize, n: u32) {
    if n == 0 { buf[*pos] = b'0'; *pos += 1; return; }
    let mut tmp = [0u8; 10];
    let mut i = 0;
    let mut n = n;
    while n > 0 { tmp[i] = b'0' + (n % 10) as u8; n /= 10; i += 1; }
    for j in (0..i).rev() { buf[*pos] = tmp[j]; *pos += 1; }
}
