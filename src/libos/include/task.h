#ifndef __OCCLUM_TASK_H__
#define __OCCLUM_TASK_H__

#ifndef __ASSEMBLY__

#ifdef __cplusplus
extern "C" {
#endif

#include <stddef.h>
#include <sys/types.h>
#include <setjmp.h>

// See Struct Task in process.rs
struct Task {
    uint64_t            kernel_stack_addr;
    uint64_t            kernel_fsbase_addr;
    uint64_t            user_stack_addr;
    uint64_t            user_fsbase_addr;
    uint64_t            user_entry_addr;
    jmp_buf*            saved_state;
};

void __set_current_task(struct Task* task);
struct Task* __get_current_task(void);

int do_run_task(struct Task* task);
void do_exit_task(void);

#ifdef __cplusplus
}
#endif

#else  /* __ASSEMBLY__ */

/* See /<path-to-linux-sgx>/common/inc/internal/thread_data.h */
#define TD_STACKGUARD_OFFSET        (8 * 5)
/* Override the field for stack guard */
#define TD_TASK_OFFSET              TD_STACKGUARD_OFFSET

#define TASK_KERNEL_STACK_ADDR      (8 * 0)
#define TASK_KERNEL_FSBASE_ADDR     (8 * 1)
#define TASK_USER_STACK_ADDR        (8 * 2)
#define TASK_USER_FSBASE_ADDR       (8 * 3)
#define TASK_USER_ENTRY_ADDR        (8 * 4)

#endif /* __ASSEMBLY__ */

#endif /* __OCCLUM_TASK_H__ */
