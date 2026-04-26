// PC Speaker драйвер
// Порты: 0x42 (PIT канал 2), 0x43 (PIT control), 0x61 (speaker gate)
//
// Поддерживает:
//   - Воспроизведение тонов заданной частоты
//   - Простые мелодии (массив нот)
//   - Базовый WAV парсер (только PCM 8-bit mono, через ШИМ)

const PIT_CHANNEL2: u16 = 0x42;
const PIT_COMMAND:  u16 = 0x43;
const SPEAKER_PORT: u16 = 0x61;
const PIT_BASE_FREQ: u32 = 1193180;

unsafe fn out8(port: u16, val: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") val);
}

unsafe fn in8(port: u16) -> u8 {
    let v: u8;
    core::arch::asm!("in al, dx", out("al") v, in("dx") port);
    v
}

/// Включить спикер на заданной частоте (Гц)
pub fn play_tone(freq: u32) {
    if freq == 0 { stop(); return; }
    let divisor = PIT_BASE_FREQ / freq;
    unsafe {
        // PIT канал 2, mode 3 (square wave), binary
        out8(PIT_COMMAND, 0xB6);
        out8(PIT_CHANNEL2, (divisor & 0xFF) as u8);
        out8(PIT_CHANNEL2, ((divisor >> 8) & 0xFF) as u8);
        // Включаем спикер (биты 0 и 1 порта 0x61)
        let val = in8(SPEAKER_PORT);
        out8(SPEAKER_PORT, val | 0x03);
    }
}

/// Выключить спикер
pub fn stop() {
    unsafe {
        let val = in8(SPEAKER_PORT);
        out8(SPEAKER_PORT, val & !0x03);
    }
}

/// Задержка в ~миллисекундах (busy wait через PIT)
pub fn delay_ms(ms: u32) {
    // Используем порт 0x80 как задержку ~1мкс на цикл
    // На 1GHz+ это неточно, но для спикера достаточно
    for _ in 0..ms * 1000 {
        unsafe { out8(0x80, 0); }
    }
}

// ── Ноты ──────────────────────────────────────────────────────────────────

pub const C4:  u32 = 262;
pub const CS4: u32 = 277;
pub const D4:  u32 = 294;
pub const DS4: u32 = 311;
pub const E4:  u32 = 330;
pub const F4:  u32 = 349;
pub const FS4: u32 = 370;
pub const G4:  u32 = 392;
pub const GS4: u32 = 415;
pub const A4:  u32 = 440;
pub const AS4: u32 = 466;
pub const B4:  u32 = 494;
pub const C5:  u32 = 523;
pub const D5:  u32 = 587;
pub const E5:  u32 = 659;
pub const F5:  u32 = 698;
pub const G5:  u32 = 784;
pub const A5:  u32 = 880;
pub const REST: u32 = 0;

/// Нота: (частота, длительность в мс)
pub type Note = (u32, u32);

/// Воспроизвести мелодию
pub fn play_melody(notes: &[Note]) {
    for &(freq, dur) in notes {
        play_tone(freq);
        delay_ms(dur);
        stop();
        delay_ms(20); // пауза между нотами
    }
}

// ── Встроенные мелодии ────────────────────────────────────────────────────

pub const MELODY_BOOT: &[Note] = &[
    (C5, 100), (E5, 100), (G5, 150),
];

pub const MELODY_ERROR: &[Note] = &[
    (A4, 200), (REST, 50), (A4, 200),
];

pub const MELODY_NOTIFY: &[Note] = &[
    (G5, 80), (E5, 80),
];

// ── WAV плеер ─────────────────────────────────────────────────────────────
// Поддерживает только: PCM, 8-bit, mono, любая частота дискретизации
// Воспроизводит через PC Speaker (очень грубо, но работает)

#[repr(C, packed)]
struct WavHeader {
    riff:       [u8; 4],  // "RIFF"
    file_size:  u32,
    wave:       [u8; 4],  // "WAVE"
    fmt_id:     [u8; 4],  // "fmt "
    fmt_size:   u32,
    audio_fmt:  u16,      // 1 = PCM
    channels:   u16,
    sample_rate: u32,
    byte_rate:  u32,
    block_align: u16,
    bits:       u16,
    data_id:    [u8; 4],  // "data"
    data_size:  u32,
}

pub enum AudioError {
    TooSmall,
    NotWav,
    NotPcm,
    NotMono8,
}

pub fn play_wav(data: &[u8]) -> Result<(), AudioError> {
    if data.len() < core::mem::size_of::<WavHeader>() {
        return Err(AudioError::TooSmall);
    }

    let hdr = unsafe { &*(data.as_ptr() as *const WavHeader) };

    if &hdr.riff != b"RIFF" || &hdr.wave != b"WAVE" {
        return Err(AudioError::NotWav);
    }
    if hdr.audio_fmt != 1 {
        return Err(AudioError::NotPcm);
    }
    if hdr.channels != 1 || hdr.bits != 8 {
        return Err(AudioError::NotMono8);
    }

    let sample_rate = hdr.sample_rate;
    let header_size = core::mem::size_of::<WavHeader>();
    let audio_data = &data[header_size..];
    let data_size = (hdr.data_size as usize).min(audio_data.len());

    // Период между сэмплами в мкс
    let period_us = 1_000_000 / sample_rate;

    // Воспроизводим: каждый сэмпл (0-255) → частота 100-4000 Гц
    // Это очень грубая аппроксимация через PC Speaker
    let step = (sample_rate / 8000).max(1) as usize; // прореживаем для скорости

    for i in (0..data_size).step_by(step) {
        let sample = audio_data[i];
        // Маппим 0-255 → 200-4000 Гц
        let freq = 200u32 + (sample as u32 * 15);
        play_tone(freq);
        // Задержка на период сэмпла
        for _ in 0..period_us * step as u32 {
            unsafe { out8(0x80, 0); }
        }
    }

    stop();
    Ok(())
}
