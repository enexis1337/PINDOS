ENTRY(_start)

SECTIONS {
    /*
     * Для BIOS загрузки: ядро загружается по 0x10000
     * Для Multiboot2/GRUB: GRUB загружает ELF по адресам из заголовков
     * Для QEMU -kernel: используем стандартный адрес 1MB
     */
    . = 0x100000;

    /* Multiboot2 заголовок — ОБЯЗАТЕЛЬНО первым, в первых 32KB */
    .multiboot2 : {
        KEEP(*(.multiboot2))
    }

    .text : {
        KEEP(*(.text._start))
        *(.text .text.*)
    }

    .rodata : ALIGN(16) {
        *(.rodata .rodata.*)
    }

    .data : ALIGN(16) {
        *(.data .data.*)
    }

    .bss : ALIGN(16) {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        __bss_end = .;
    }

    /DISCARD/ : {
        *(.eh_frame .eh_frame_hdr)
        *(.note .note.*)
        *(.comment)
        *(.gnu.*)
    }
}
