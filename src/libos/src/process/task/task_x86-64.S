#define __ASSEMBLY__
#include "task.h"

    .file "task_x86-64.S"

    .global __set_current_task
    .type __set_current_task, @function
__set_current_task:
    movq %rdi, %gs:(TD_TASK_OFFSET)
    ret

    .global __get_current_task
    .type __get_current_task, @function
__get_current_task:
    movq %gs:(TD_TASK_OFFSET), %rax
    ret

    .global __get_stack_guard
    .type __get_stack_guard, @function
__get_stack_guard:
    movq %gs:(TD_TASK_OFFSET), %rax
    ret

    .global __set_stack_guard
    .type __set_stack_guard, @function
__set_stack_guard:
    mov %rdi, %gs:(TD_TASK_OFFSET)
    ret

    .global __exec_task
    .type __exec_task, @function
__exec_task:
    // Save kernel fsbase and use user fsbase
    //
    // SGX HW Mode and SIM Mode require different implementations. In SGX hardware
    // mode, we read/write fsbase via RDFSBASE/WRFSBASE instruction directly.
    // But in SGX simulation mode, modifying fsbase directly via the instrution will
    // break GDB (ptrace can't get right value if WRFSBASE is called which
    // will make debugger fail in simulation mode). Thus we read/write FS base via
    // arch_prctl system call.
#if SGX_MODE_SIM
    pushq %rdi
    pushq %rsi
    movq %rdi, %r10
    movq %rdi, %r12

    // Save kernel fsbase
    movq $ARCH_GET_FS, %rdi
    add $TASK_KERNEL_FS, %r10
    movq %r10, %rsi
    call __arch_prctl

    // Use user fsbase
    movq $ARCH_SET_FS, %rdi
    movq TASK_USER_FS(%r12), %rsi
    call __arch_prctl

    popq %rsi
    popq %rdi
#else // SGX_MODE_HW
    // Save kernel fsbase
    rdfsbase %r10
    movq %r10, TASK_KERNEL_FS(%rdi)
    // Use user fsbase
    movq TASK_USER_FS(%rdi), %r10
    wrfsbase %r10
#endif

    // Use user stack
    movq TASK_USER_RSP(%rdi), %rsp
    // Run user code
    movq TASK_USER_ENTRY_ADDR(%rdi), %r11
    jmp *%r11
