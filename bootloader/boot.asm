; PINDOS bootloader — Stage 1 (MBR, 512 байт)
; Загружает Stage 2 + ядро, инициализирует VESA, переходит в PM
[BITS 16]
[ORG 0x7C00]

KERNEL_LOAD_SEG  equ 0x1000   ; ядро по 0x10000
KERNEL_SECTORS   equ 200      ; секторов ядра
KERNEL_LBA_START equ 1        ; LBA ядра (сразу после MBR)
VESA_INFO_ADDR   equ 0x7E00   ; куда сохраняем VESA info

start:
    cli
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7C00
    
    ; DEBUG: пишем 'B' в левый верхний угол экрана
    mov byte [0xB8000], 'B'
    mov byte [0xB8001], 0x0F
    
    mov [boot_drive], dl
    sti

    ; Загружаем ядро
    mov ah, 0x41
    mov bx, 0x55AA
    mov dl, [boot_drive]
    int 0x13
    jc .use_chs
    cmp bx, 0xAA55
    jne .use_chs
    test cx, 1
    jz .use_chs
    call load_lba
    jmp .loaded
.use_chs:
    call load_chs
.loaded:

    ; VESA инициализация
    mov cx, 0x118       ; 1024x768x32
    call try_vesa
    jnc .vesa_done
    mov cx, 0x115       ; 800x600x32
    call try_vesa
    jnc .vesa_done
    mov cx, 0x112       ; 640x480x32
    call try_vesa
.vesa_done:

    ; Protected mode
    cli
    lgdt [gdt_desc]
    mov eax, cr0
    or  eax, 1
    mov cr0, eax
    jmp 0x08:pm32

; ── LBA ───────────────────────────────────────────────────────────────────
load_lba:
    mov word  [dap],    0x0010
    mov word  [dap+2],  KERNEL_SECTORS
    mov word  [dap+4],  0x0000
    mov word  [dap+6],  KERNEL_LOAD_SEG
    mov dword [dap+8],  KERNEL_LBA_START
    mov dword [dap+12], 0
    mov ah, 0x42
    mov dl, [boot_drive]
    mov si, dap
    int 0x13
    jc .err
    ret
.err: cli
    hlt

; ── CHS ───────────────────────────────────────────────────────────────────
load_chs:
    mov ah, 0x02
    mov al, KERNEL_SECTORS
    mov ch, 0
    mov cl, 2
    mov dh, 0
    mov dl, [boot_drive]
    mov bx, KERNEL_LOAD_SEG
    mov es, bx
    xor bx, bx
    int 0x13
    jc .err
    ret
.err: cli
    hlt

; ── VESA: пробуем режим CX ────────────────────────────────────────────────
try_vesa:
    push cx
    push es
    push di
    xor ax, ax
    mov es, ax
    mov di, 0x8000
    mov ax, 0x4F01
    int 0x10
    pop di
    pop es
    cmp ax, 0x004F
    jne .fail
    mov bx, [0x8000]
    test bx, 0x0081     ; present + LFB
    jz .fail
    pop cx
    push cx
    mov ax, 0x4F02
    mov bx, cx
    or  bx, 0x4000
    int 0x10
    cmp ax, 0x004F
    jne .fail2
    ; Сохраняем: signature, addr, width, height, pitch, bpp
    mov dword [VESA_INFO_ADDR],    0x56455341
    mov eax,  [0x8028]
    mov       [VESA_INFO_ADDR+4],  eax
    mov ax,   [0x8012]
    mov       [VESA_INFO_ADDR+8],  ax
    mov ax,   [0x8014]
    mov       [VESA_INFO_ADDR+10], ax
    mov ax,   [0x8010]
    mov       [VESA_INFO_ADDR+12], ax
    mov al,   [0x8019]
    mov       [VESA_INFO_ADDR+14], al
    pop cx
    clc
    ret
.fail2:
    pop cx
    stc
    ret
.fail:
    pop cx
    stc
    ret

; ── PM32 ──────────────────────────────────────────────────────────────────
[BITS 32]
pm32:
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    mov esp, 0x9F000
    mov byte [0xB8000], 'P'
    mov byte [0xB8001], 0x0F
    jmp 0x10000

; ── GDT ───────────────────────────────────────────────────────────────────
[BITS 16]
gdt_start:
    dq 0x0000000000000000
    dq 0x00CF9A000000FFFF
    dq 0x00CF92000000FFFF
gdt_end:
gdt_desc:
    dw gdt_end - gdt_start - 1
    dd gdt_start

; ── DAP ───────────────────────────────────────────────────────────────────
dap: times 16 db 0

boot_drive db 0

times 510 - ($ - $$) db 0
dw 0xAA55
