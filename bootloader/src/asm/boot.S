.code32

.section .text.boot, "ax"
.align 8

/* Multiboot2 header */
.set MAGIC,         0xE85250D6
.set ARCH,          0
.set HEADER_LEN,    header_end - header_start
.set CHECKSUM,      0x100000000 - (MAGIC + ARCH + HEADER_LEN)

header_start:
    .long MAGIC
    .long ARCH
    .long HEADER_LEN
    .long CHECKSUM
    .word 0
    .word 0
    .long 8
header_end:

.globl _start
_start:
    /* GRUB loads us in 32-bit protected mode */
    mov $stack_top, %esp
    cli

    /* Check for CPUID + long mode support */
    pushfl
    popl %eax
    movl %eax, %ecx
    xorl $0x200000, %eax
    pushl %eax
    popfl
    pushfl
    popl %eax
    xorl %ecx, %eax
    jz no_long_mode
    pushfl

    movl $0x80000000, %eax
    cpuid
    cmpl $0x80000001, %eax
    jb no_long_mode

    movl $0x80000001, %eax
    cpuid
    testl $0x20000000, %edx
    jz no_long_mode

    /* Set up page tables for identity mapping 0-16MB using 2MB pages */
    movl $pml4, %eax
    movl %eax, %cr3

    /* Enable PAE */
    movl %cr4, %eax
    orl $0x20, %eax
    movl %eax, %cr4

    /* Enable long mode via EFER MSR */
    movl $0xC0000080, %ecx
    rdmsr
    orl $0x100, %eax
    wrmsr

    /* Enable paging + protected mode */
    movl %cr0, %eax
    orl $0x80000001, %eax
    movl %eax, %cr0

    /* Far jump to 64-bit code */
    lgdt gdt64_ptr
    ljmp $0x08, $_start64

.code64
_start64:
    movw $0x10, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %fs
    movw %ax, %gs
    movw %ax, %ss

    /* Jump to Rust code */
    jmp boot_main

no_long_mode:
    movl $0xB8000, %edi
    movl $0x04F04F04, (%edi)
    movl $0x04F04F04, 4(%edi)
    movl $0x04F04F04, 8(%edi)
    movl $0x04F04F04, 12(%edi)
    hlt
    jmp no_long_mode

.section .data.boot, "aw", @progbits
.align 16
gdt64:
    .quad 0
    .quad (1<<43) | (1<<44) | (1<<47) | (1<<53)
    .quad (1<<44) | (1<<47) | (1<<53)
gdt64_ptr:
    .word gdt64_ptr - gdt64 - 1
    .quad gdt64

/* Page tables: identity map 0-16MB using 2MB huge pages */
.align 4096
pml4:
    .quad pdpt + 3
    .zero 4088

.align 4096
pdpt:
    .quad pd + 3
    .zero 4088

.align 4096
pd:
    .set i, 0
    .rept 8
    .quad (i * 0x200000) | 0x83
    .set i, i + 1
    .endr
    .zero 4032

/* Stack (16KB) */
.align 16
    .space 16384
stack_top:
