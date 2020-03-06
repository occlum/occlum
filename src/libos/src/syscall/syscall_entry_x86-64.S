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
    movq TASK_KERNEL_RSP(%r12), %rsp

    // Use kernel fsbase. Different implementation for HW and SIM.
#if SGX_MODE_SIM
    pushq %rdi
    pushq %rsi

    movq $ARCH_SET_FS, %rdi
    movq TASK_KERNEL_FS(%r12), %rsi
    call __arch_prctl

    popq %rsi
    popq %rdi
#else // SGX_MODE_HW
    movq TASK_KERNEL_FS(%r12), %r11
    wrfsbase %r11
#endif

    // Use kernel stack base and limit
    movq TASK_KERNEL_STACK_BASE(%r12), %r11
    movq %r11, %gs:TD_STACK_BASE

    movq TASK_KERNEL_STACK_LIMIT(%r12), %r11
    movq %r11, %gs:TD_STACK_LIMIT

    // Make %rsp 16-byte aligned before call
    sub $0x8, %rsp
    // Pass arg5
    pushq 0x18(%rbp)

    call dispatch_syscall

    // Use user fsbase. Different implementation for HW and SIM.
#if SGX_MODE_SIM
    pushq %rdi
    pushq %rsi
    pushq %rax // RAX must be saved here otherwise the progrom may crash.

    movq $ARCH_SET_FS, %rdi
    movq TASK_USER_FS(%r12), %rsi
    call __arch_prctl

    popq %rax
    popq %rsi
    popq %rdi
#else // SGX_MODE_HW
    movq TASK_USER_FS(%r12), %r11
    wrfsbase %r11
#endif

    // Use user stack base and limit
    movq TASK_USER_STACK_BASE(%r12), %r11
    movq %r11, %gs:TD_STACK_BASE

    movq TASK_USER_STACK_LIMIT(%r12), %r11
    movq %r11, %gs:TD_STACK_LIMIT

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
