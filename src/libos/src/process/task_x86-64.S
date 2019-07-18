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

    .global __run_task
    .type __run_task, @function
__run_task:
    // Save kernel fsbase
    rdfsbase %r10
    movq %r10, TASK_KERNEL_FS(%rdi)
    // Use user fsbase
    movq TASK_USER_FS(%rdi), %r10
    wrfsbase %r10
    // Use user stack
    movq TASK_USER_RSP(%rdi), %rsp
    // Run user code
    movq TASK_USER_ENTRY_ADDR(%rdi), %r11
    jmp *%r11
