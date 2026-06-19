; Simple bootloader stub for QEMU
; Multiboot2 header для прямого запуска через QEMU -kernel

format elf64

; Multiboot2 header
mb2_header:
    dd 0xe85250d6           ; magic
    dd 0                    ; architecture (i386)
    dd mb2_header_end - mb2_header  ; header length
    dd -(0xe85250d6 + 0 + (mb2_header_end - mb2_header))  ; checksum
    
    ; End tag
    dw 0
    dw 0
    dd 8

mb2_header_end:

; Entry point
public _start
section '.text' executable
_start:
    ; Just halt for now - the actual _start is in Rust
    cli
    hlt
