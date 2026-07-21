OUTPUT_FORMAT(elf64-x86-64)
ENTRY(_start)

PHDRS
{
    text    PT_LOAD    FLAGS(0x05);
    rodata  PT_LOAD    FLAGS(0x04);
    data    PT_LOAD    FLAGS(0x06);
    bss     PT_LOAD    FLAGS(0x06);
}

SECTIONS
{
    . = 2M;

    .text : {
        *(.text.boot)
        *(.text .text.*)
    } :text

    .rodata : {
        *(.rodata .rodata.*)
    } :rodata

    .data : {
        *(.data .data.*)
    } :data

    .bss : {
        *(COMMON)
        *(.bss .bss.*)
    } :bss

    .got : {
        *(.got)
    } :data

    .got.plt : {
        *(.got.plt)
    } :data

    /DISCARD/ : {
        *(.eh_frame)
        *(.eh_frame_hdr)
        *(.comment)
    }
}
