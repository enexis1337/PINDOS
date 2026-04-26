; PINDOS kernel entry point
; Поддерживает три способа загрузки:
;   1. Прямой BIOS/CSM — прыжок с 0x10000 (наш загрузчик)
;   2. Multiboot1      — через QEMU -kernel или GRUB legacy
;   3. Multiboot2      — через GRUB2 (UEFI + BIOS)

[BITS 32]

global _start
global stack_top
extern kernel_main
extern __bss_start
extern __bss_end

; ── Multiboot1 заголовок (для QEMU -kernel) ───────────────────────────────
; БЕЗ флага бит 2 (VBE) — QEMU -kernel его не поддерживает
; Bochs VGA инициализируем сами через порты 0x01CE/0x01CF
MB1_MAGIC    equ 0x1BADB002
MB1_FLAGS    equ 0x00000000
MB1_CHECKSUM equ (0x100000000 - MB1_MAGIC - MB1_FLAGS)

section .multiboot2
align 4
    dd MB1_MAGIC
    dd MB1_FLAGS
    dd MB1_CHECKSUM

; Multiboot2 временно отключён для отладки
; MB2_MAGIC    equ 0xE85250D6
; MB2_ARCH     equ 0
; MB2_LENGTH   equ (mb2_end - mb2_start)
; MB2_CHECKSUM equ (0x100000000 - (MB2_MAGIC + MB2_ARCH + MB2_LENGTH))
; 
; align 8
; mb2_start:
;     dd MB2_MAGIC
;     dd MB2_ARCH
;     dd MB2_LENGTH
;     dd MB2_CHECKSUM
; 
;     align 8
;     dw 6        ; memory map
;     dw 0
;     dd 8
; 
;     align 8
;     dw 5        ; framebuffer (optional)
;     dw 1
;     dd 20
;     dd 0
;     dd 80
;     dd 25
;     dd 0
; 
;     align 8
;     dw 0        ; end tag
;     dw 0
;     dd 8
; mb2_end:

; ── Точка входа ───────────────────────────────────────────────────────────
section .text._start
_start:
    ; Немедленно запрещаем прерывания — IDT ещё не загружена,
    ; аппаратные IRQ (таймер и др.) вызовут Triple Fault
    cli

    ; EAX содержит magic загрузчика:
    ;   0x36D76289 — Multiboot2 (GRUB2)
    ;   0x2BADB002 — Multiboot1 (QEMU -kernel, GRUB legacy)
    ;   иначе      — наш BIOS загрузчик
    cmp eax, 0x36D76289
    je .multiboot2_entry
    cmp eax, 0x2BADB002
    je .multiboot1_entry

    ; ── BIOS загрузка ─────────────────────────────────────────────────────
.bios_entry:
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    mov esp, stack_top
    jmp .common_init

    ; ── Multiboot1 загрузка (QEMU -kernel) ───────────────────────────────
.multiboot1_entry:
    ; EBX = Multiboot1 info (игнорируем, mb2_info_ptr = 0)
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    mov esp, stack_top
    jmp .common_init

    ; ── Multiboot2 загрузка ───────────────────────────────────────────────
.multiboot2_entry:
    mov [mb2_info_ptr], ebx
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    mov esp, stack_top

.common_init:
    ; Очищаем BSS (используем символы линкера — правильный диапазон)
    mov edi, __bss_start
    mov ecx, __bss_end
    sub ecx, edi
    xor eax, eax
    rep stosb

    ; Передаём указатель на MB2 info в ядро (через стек)
    push dword [mb2_info_ptr]
    call kernel_main
    ; kernel_main никогда не возвращается (-> !)
    ; Но на случай бага — зависаем здесь

.halt:
    cli
    hlt
    jmp .halt

; ── Данные ────────────────────────────────────────────────────────────────
section .data
mb2_info_ptr: dd 0

section .bss
alignb 4
    resb 4   ; placeholder — реальный BSS из Rust через линкерные символы __bss_start/__bss_end

; Стек ядра — в конце BSS, чтобы rep stosb его не затёр
alignb 16
    resb 0x10000  ; 64KB стек
stack_top:
