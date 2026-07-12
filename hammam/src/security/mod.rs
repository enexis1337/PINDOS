use crate::kprintln;
use core::sync::atomic::{AtomicU64, Ordering};

/// KASLR — Kernel Address Space Layout Randomization
/// Рандомизация базы ядра для защиты от ROP атак.
/// Вызывается из bootloader ДО перехода в ядро.
/// Использует RDRAND для получения случайного смещения,
/// выравнивает на 2 MiB (размер huge page).
pub fn kaslr_offset() -> u64 {
    let mut rand: u64;
    // SAFETY: RDRAND поддерживается на всех целевых x86-64 CPU
    unsafe {
        core::arch::asm!(
            "rdrand {}",
            out(reg) rand,
            options(nostack)
        );
    }
    // Выровнять на 2 MiB (0x200000) и обнулить старшие биты
    rand & 0x00FF_FFFF_FFF0_0000
}

fn check_cpuid_leaf07_ebx() -> u64 {
    let ebx: u64;
    unsafe {
        core::arch::asm!(
            "push rbx",
            "mov eax, 7",
            "xor ecx, ecx",
            "cpuid",
            "mov {0}, rbx",
            "pop rbx",
            out(reg) ebx,
            out("eax") _,
            out("ecx") _,
            lateout("edx") _,
            options(preserves_flags)
        );
    }
    ebx
}

/// SMEP (Supervisor Mode Execution Protection) + SMAP (Supervisor Mode Access Prevention)
/// Запретить ядру исполнять и читать userspace память.
/// Вызывается из _start на каждом CPU после инициализации GDT.
pub fn enable_smep_smap() {
    let ebx = check_cpuid_leaf07_ebx();
    let has_smep = (ebx & (1 << 7)) != 0;
    let has_smap = (ebx & (1 << 20)) != 0;

    if !has_smep && !has_smap {
        kprintln!("[WARN] CPU does not support SMEP or SMAP");
        return;
    }

    unsafe {
        let cr4: u64;
        core::arch::asm!("mov {}, cr4", out(reg) cr4, options(nostack, preserves_flags));

        let mut new_cr4 = cr4;
        if has_smep {
            new_cr4 |= 1 << 20;
        }
        if has_smap {
            new_cr4 |= 1 << 21;
        }

        if new_cr4 != cr4 {
            core::arch::asm!("mov cr4, {}", in(reg) new_cr4, options(nostack, preserves_flags));
        }
    }
}

/// Enable SSE/SSE2 in CR4 and initialize MXCSR.
/// Required for userspace programs compiled with SSE instructions.
pub fn enable_sse() {
    unsafe {
        let cr4: u64;
        core::arch::asm!("mov {}, cr4", out(reg) cr4, options(nostack, preserves_flags));
        let new_cr4 = cr4 | (1 << 9) | (1 << 10); // OSFXSR | OSXMMEXCPT
        if new_cr4 != cr4 {
            core::arch::asm!("mov cr4, {}", in(reg) new_cr4, options(nostack, preserves_flags));
        }
        // Initialize MXCSR to default (all exceptions masked, round-to-nearest)
        core::arch::asm!(
            "ldmxcsr [{0}]",
            in(reg) &0x1F80u32 as *const u32,
            options(nostack, preserves_flags)
        );
        // Initialize x87 FPU
        core::arch::asm!("fninit", options(nostack, preserves_flags));
    }
}

/// NX (No-Execute) / XD (Execute Disable)
/// Запретить исполнение кода в data-страницах.
/// Включить бит NXE (No-Execute Enable) в MSR_EFER.
/// Это требует поддержки PAE + NXE в процессоре.
pub fn enable_nx() {
    unsafe {
        let efer_low: u32;
        let efer_high: u32;

        // Читаем MSR_EFER (0xC0000080)
        core::arch::asm!(
            "rdmsr",
            in("ecx") 0xC0000080u32,
            out("eax") efer_low,
            out("edx") efer_high,
            options(nostack, preserves_flags)
        );

        let mut efer = ((efer_high as u64) << 32) | (efer_low as u64);

        // Устанавливаем бит NXE (бит 11)
        efer |= 1 << 11;

        // Пишем обратно в MSR_EFER
        core::arch::asm!(
            "wrmsr",
            in("ecx") 0xC0000080u32,
            in("eax") efer as u32,
            in("edx") (efer >> 32) as u32,
            options(nostack, preserves_flags)
        );
    }
}

/// Stack Canary — глобальное значение для защиты от переполнения стека.
/// Инициализируется через RDRAND при boot.
/// Компилятор может вставить проверку этого значения перед возвратом.
pub static STACK_CANARY: AtomicU64 = AtomicU64::new(0);

fn cpuid_ecx_leaf01() -> u64 {
    let ecx: u64;
    unsafe {
        core::arch::asm!(
            "push rbx",
            "mov eax, 1",
            "xor ecx, ecx",
            "cpuid",
            "mov {0}, rcx",
            "pop rbx",
            out(reg) ecx,
            out("eax") _,
            lateout("edx") _,
            options(preserves_flags)
        );
    }
    ecx
}

/// Инициализировать stack canary случайным значением
pub fn init_canary() {
    let ecx = cpuid_ecx_leaf01();
    let has_rdrand = (ecx & (1 << 30)) != 0;

    if !has_rdrand {
        kprintln!("[WARN] CPU does not support RDRAND, using fallback canary");
        STACK_CANARY.store(0xDEAD_BEEF_CAFE_BABE, Ordering::Release);
        return;
    }

    let mut val: u64;
    unsafe {
        core::arch::asm!(
            "rdrand {}",
            out(reg) val,
            options(nostack)
        );
    }
    STACK_CANARY.store(val, Ordering::Release);
}

/// CFI (Control Flow Integrity) — заполнитель для будущей реализации
/// Могла бы использовать CET (Control-flow Enforcement Technology) если доступна
pub fn enable_cfi() {
    // TODO: включить CET если поддерживается процессором
    // - Читать CPUID для проверки наличия CET
    // - Установить биты в CR4 для включения CET
    // - Инициализировать Shadow Stack
}

/// IBRS/IBPB — защита от Spectre атак
/// Indirect Branch Restricted Speculation (IBRS)
/// Indirect Branch Prediction Barrier (IBPB)
pub fn enable_spectre_mitigations() {
    unsafe {
        // Читаем MSR_IA32_SPEC_CTRL (0x48)
        let spec_ctrl_low: u32;
        let spec_ctrl_high: u32;
        core::arch::asm!(
            "rdmsr",
            in("ecx") 0x48u32,
            out("eax") spec_ctrl_low,
            out("edx") spec_ctrl_high,
            options(nostack, preserves_flags)
        );

        let mut spec_ctrl = ((spec_ctrl_high as u64) << 32) | (spec_ctrl_low as u64);

        // Устанавливаем IBRS (бит 0)
        spec_ctrl |= 1 << 0;

        // Пишем обратно
        core::arch::asm!(
            "wrmsr",
            in("ecx") 0x48u32,
            in("eax") spec_ctrl as u32,
            in("edx") (spec_ctrl >> 32) as u32,
            options(nostack, preserves_flags)
        );
    }
}

/// STIBP — Single Thread Indirect Branch Prediction
/// Защита от Spectre V4 в многопоточной среде
pub fn enable_stibp() {
    unsafe {
        // Читаем MSR_IA32_SPEC_CTRL
        let spec_ctrl_low: u32;
        let spec_ctrl_high: u32;
        core::arch::asm!(
            "rdmsr",
            in("ecx") 0x48u32,
            out("eax") spec_ctrl_low,
            out("edx") spec_ctrl_high,
            options(nostack, preserves_flags)
        );

        let mut spec_ctrl = ((spec_ctrl_high as u64) << 32) | (spec_ctrl_low as u64);

        // Устанавливаем STIBP (бит 1)
        spec_ctrl |= 1 << 1;

        // Пишем обратно
        core::arch::asm!(
            "wrmsr",
            in("ecx") 0x48u32,
            in("eax") spec_ctrl as u32,
            in("edx") (spec_ctrl >> 32) as u32,
            options(nostack, preserves_flags)
        );
    }
}

/// Инициализировать все механизмы защиты ядра
/// Вызывается из _start после инициализации GDT
pub fn init_all() {
    enable_smep_smap();
    enable_nx();
    init_canary();
    enable_spectre_mitigations();
    enable_stibp();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kaslr_offset() {
        let offset1 = kaslr_offset();
        let offset2 = kaslr_offset();

        // Offsets должны быть выровнены на 2 MiB
        assert_eq!(offset1 & 0x1FFFFF, 0, "KASLR offset должен быть выровнен на 2 MiB");
        assert_eq!(offset2 & 0x1FFFFF, 0, "KASLR offset должен быть выровнен на 2 MiB");

        // Вероятно (99.99%), что два вызова дадут разные значения
        // (нельзя гарантировать, но весьма вероятно)
        // assert_ne!(offset1, offset2, "KASLR offsets должны быть разными");
    }

    #[test]
    fn test_stack_canary() {
        let initial = STACK_CANARY.load(Ordering::Acquire);
        init_canary();
        let after_init = STACK_CANARY.load(Ordering::Acquire);

        // После инициализации canary должен быть ненулевым
        assert_ne!(after_init, 0, "Stack canary не должен быть нулевым после инициализации");

        // Вероятно, что после инициализации значение изменится
        // (нельзя гарантировать, если начальное было 0)
    }

    #[test]
    fn test_canary_is_atomic() {
        // Проверяем, что STACK_CANARY — это действительно AtomicU64
        let canary = STACK_CANARY.load(Ordering::SeqCst);
        STACK_CANARY.store(canary, Ordering::SeqCst);

        let canary2 = STACK_CANARY.load(Ordering::SeqCst);
        assert_eq!(canary, canary2);
    }
}
