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
    mov %rsi, %rsp
    jmp *%rdi
