#include "task.h"

extern void __run_task(uint64_t entry_point, uint64_t stack_top);

extern uint64_t __get_stack_guard(void);
extern void __set_stack_guard(uint64_t new_val);

static uint64_t get_syscall_stack(struct Task* this_task) {
#define LARGE_ENOUGH_GAP        4096
    char libos_stack_var = 0;
    uint64_t libos_stack = ((uint64_t) &libos_stack_var) - LARGE_ENOUGH_GAP;
    libos_stack &= ~0x0FUL; // stack must be 16-byte aligned
    return libos_stack;
}

#define SET_CURRENT_TASK(task)                  \
    long stack_guard = __get_stack_guard();     \
    __set_current_task(task);

#define RESET_CURRENT_TASK()                    \
    __set_stack_guard(stack_guard);


int do_run_task(struct Task* task) {
    jmp_buf libos_state = {0};
    task->saved_state = &libos_state;
    task->syscall_stack_addr = get_syscall_stack(task);

    SET_CURRENT_TASK(task);

    int second = setjmp(libos_state);
    if (!second) {
        __run_task(task->user_entry_addr, task->user_stack_addr);
    }

    // From occlum_exit
    RESET_CURRENT_TASK();
    return 0;
}

void do_exit_task(void) {
    jmp_buf* jb = __get_current_task()->saved_state;
    longjmp(*jb, 1);
}
