#define __ASSEMBLY__
#include "task.h"

    .file "syscall_entry_x86-64.S"
    .global __occlum_syscall
    .type __occlum_syscall, @function
__occlum_syscall:
    //      num  - %rdi
    //      arg0 - %rsi
    //      arg1 - %rdx
    //      arg2 - %rcx
    //      arg3 - %r8
    //      arg4 - %r9
    //      arg5 - *0x8(%rsp)

    // Given by the user, the user-space stack pointer %rsp cannot be trusted.
    // So we need to check whether %rsp is within the read-write region of the
    // current data domain
    bndcl %rsp, %bnd0
    bndcu %rsp, %bnd0

    // Save the callee-saved registers
    pushq %rbp
    pushq %r12
    // Save the user stack
    movq %rsp, %rbp

    // Get current task
    movq %gs:(TD_TASK_OFFSET), %r12
    // Switch to the kernel stack
    movq TASK_KERNEL_STACK_ADDR(%r12), %rsp
    // Use kernel fsbase
    movq TASK_KERNEL_FSBASE_ADDR(%r12), %r11
    wrfsbase %r11

    // TODO: Pass arg5
    // pushq 0x10(%rbp)

    call dispatch_syscall

    // addq 0x08, %rsp

    // Use user fsbase
    movq TASK_USER_FSBASE_ADDR(%r12), %r11
    wrfsbase %r11

    // Switch to the user stack
    movq %rbp, %rsp
    // Restore callee-saved registers
    popq %r12
    popq %rbp

    // Check return target is a valid instruction (i.e., a cfi_label)
    popq %r10
    movq (%r10), %r11
    bndcl %r11, %bnd2
    bndcu %r11, %bnd2
    jmpq *%r10
