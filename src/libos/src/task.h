#ifndef __RUSGX_TASK_H__
#define __RUSGX_TASK_H__

#ifndef __ASSEMBLY__

#ifdef __cplusplus
extern "C" {
#endif

#include <stddef.h>
#include <sys/types.h>
#include <setjmp.h>

// See Struct Task in process.rs
struct Task {
    uint32_t            pid;
    int32_t             exit_code;
    uint64_t            syscall_stack_addr;
    uint64_t            user_stack_addr;
    uint64_t            user_entry_addr;
    uint64_t            fs_base_addr;
    jmp_buf*            saved_state;
};

void __set_current_task(struct Task* task);
struct Task* __get_current_task(void);

int do_run_task(struct Task* task);
void do_exit_task(int exitcode);

#ifdef __cplusplus
}
#endif

#else  /* __ASSEMBLY__ */

/* See /<path-to-linux-sgx>/common/inc/internal/thread_data.h */
#define TD_STACKGUARD_OFFSET        (8 * 5)
/* Override the field for stack guard */
#define TD_TASK_OFFSET              TD_STACKGUARD_OFFSET

#define TASK_SYSCALL_STACK_OFFSET   (8 * 1)

#endif /* __ASSEMBLY__ */

#endif /* __RUSGX_TASK_H__ */
