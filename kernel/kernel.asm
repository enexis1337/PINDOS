; PINDOS kernel entry point
; Передаёт управление Rust ядру

[BITS 32]
[ORG 0x10000]

global _start
extern kernel_main

section .text
_start:
    ; Настраиваем стек
    mov esp, 0x90000

    ; Очищаем BSS
    mov edi, bss_start
    mov ecx, bss_end
    sub ecx, edi
    xor eax, eax
    rep stosb

    ; Вызываем Rust ядро
    call kernel_main

    ; Если вернулись — зависаем
.halt:
    cli
    hlt
    jmp .halt

section .bss
bss_start:
    resb 0x1000
bss_end:
