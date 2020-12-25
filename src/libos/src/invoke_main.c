#include "task.h"
#include "invoke_main.h"

/* See /<path-to-linux-sgx>/common/inc/internal/thread_data.h */
typedef struct _thread_data_t {
    uint64_t  reserved1[2];
    uint64_t  stack_base_addr;
    uint64_t  stack_limit_addr;
    uint64_t  reserved2[15];
    uint64_t  stack_commit_addr;
} thread_data_t;

extern thread_data_t *get_thread_data(void);

extern void __exec_task(struct Task *task);

extern uint64_t __get_stack_guard(void);
extern void __set_stack_guard(uint64_t new_val);

extern void init_occlum_syscall(void);

static uint64_t get_syscall_stack(struct Task *this_task) {
#define LARGE_ENOUGH_GAP        (8192)
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

int occlum_ecall_invoke_main(void)
{
    // use a fake/dummy task
    struct Task task = {0};

    thread_data_t *td = get_thread_data();
    task.kernel_rsp = get_syscall_stack(&task);
    task.kernel_stack_base = td->stack_base_addr;
    task.kernel_stack_limit = td->stack_limit_addr;

    SET_CURRENT_TASK(&task);

    // set occlum syscall entry in libc
    init_occlum_syscall();

    // call into the main func of app
    main();

    RESET_CURRENT_TASK();
    return 0;
}
