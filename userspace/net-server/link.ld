OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)
ENTRY(_start)

SECTIONS
{
    . = 0x400000;

    .text : {
        KEEP(*(.text.start))
        *(.text .text.*)
    }
    .rodata : { *(.rodata .rodata.*) }
    .data : { *(.data .data.*) }
    .got  : { *(.got) *(.got.plt) }
    .bss  : { *(.bss .bss.*) }

    /DISCARD/ : { *(.interp) *(.dynsym) *(.dynstr) *(.gnu.hash) *(.hash) *(.dynamic) *(.rela.*) }
}
