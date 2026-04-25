; PINDOS bootloader
; BIOS загружает этот код по адресу 0x7C00

[BITS 16]
[ORG 0x7C00]

start:
    cli
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7C00
    sti

    ; Выводим приветствие
    mov si, msg_boot
    call print_string

    ; Загружаем ядро с диска (сектор 2, 32 сектора)
    mov ah, 0x02        ; функция чтения
    mov al, 32          ; количество секторов
    mov ch, 0           ; цилиндр 0
    mov cl, 2           ; сектор 2
    mov dh, 0           ; головка 0
    mov dl, 0x80        ; первый жёсткий диск
    mov bx, 0x1000      ; адрес загрузки ES:BX
    mov es, bx
    xor bx, bx
    int 0x13
    jc disk_error

    ; Переходим в защищённый режим
    lgdt [gdt_descriptor]
    mov eax, cr0
    or eax, 1
    mov cr0, eax
    jmp 0x08:protected_mode

disk_error:
    mov si, msg_disk_err
    call print_string
    hlt

print_string:
    lodsb
    or al, al
    jz .done
    mov ah, 0x0E
    int 0x10
    jmp print_string
.done:
    ret

[BITS 32]
protected_mode:
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    mov esp, 0x90000

    ; Прыгаем в ядро
    jmp 0x10000

; GDT
gdt_start:
    dq 0x0000000000000000   ; null дескриптор
    dq 0x00CF9A000000FFFF   ; code сегмент
    dq 0x00CF92000000FFFF   ; data сегмент
gdt_end:

gdt_descriptor:
    dw gdt_end - gdt_start - 1
    dd gdt_start

msg_boot     db 'PINDOS booting...', 13, 10, 0
msg_disk_err db 'Disk read error!', 13, 10, 0

times 510 - ($ - $$) db 0
dw 0xAA55
