OUTPUT_FORMAT(elf64-x86-64)
ENTRY(_start)

PHDRS
{
    text    PT_LOAD    FLAGS(5);
    data    PT_LOAD    FLAGS(6);
}

SECTIONS
{
    . = 2M;

    .text : ALIGN(4K) {
        KEEP(*(.text.boot))
        *(.text .text.*)
    } :text

    .rodata : ALIGN(4K) {
        *(.rodata .rodata.*)
    } :text

    .data : ALIGN(4K) {
        KEEP(*(.data.boot))
        *(.data .data.*)
    } :data

    .got : ALIGN(4K) {
        *(.got)
        *(.got.plt)
    } :data

    .bss : ALIGN(4K) {
        *(COMMON)
        *(.bss .bss.*)
    } :data

    /DISCARD/ : {
        *(.eh_frame)
        *(.eh_frame_hdr)
        *(.comment)
        *(.note*)
    }
}
