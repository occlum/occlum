#define __ASSEMBLY__
#include "task.h"

    .file "syscall_entry_native_x86-64.S"
    .global __occlum_syscall_native
    .type __occlum_syscall_native, @function
__occlum_syscall_native:
    push %rbp
    movq %rsp,%rbp

    // Save registers
    pushq %rdi
    pushq %rsi
    pushq %rdx
    pushq %r10
    pushq %r8
    // arg5
    pushq %r9
    // arg4--arg0
    movq %r8, %r9
    movq %r10, %r8
    movq %rdx, %rcx
    movq %rsi, %rdx
    movq %rdi, %rsi
    // num
    movq %rax, %rdi

    // num  - %rdi
    // arg0 - %rsi
    // arg1 - %rdx
    // arg2 - %rcx
    // arg3 - %r8
    // arg4 - %r9
    // arg5 - *0x8(%rsp)
    call __occlum_syscall

    // Restore registers
    popq %r9
    popq %r8
    popq %r10
    popq %rdx
    popq %rsi
    popq %rdi

    popq %rbp
    ret
