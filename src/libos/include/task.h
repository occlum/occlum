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
    uint64_t            kernel_rsp;
    uint64_t            kernel_stack_base;
    uint64_t            kernel_stack_limit;
    uint64_t            kernel_fs;
    uint64_t            user_rsp;
    uint64_t            user_stack_base;
    uint64_t            user_stack_limit;
    uint64_t            user_fs;
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

/* Big enough offset, which is not overlap with SDK */
/*In SGX SDK the GS register point to thread_data_t structure and a whole page is
assigned to the structure. So any offset larger than sizeof(thread_data_t) and
less than 4096 is unused by anyone. We can use it.*/
#define TD_SYSCALL_RET_ADDR_OFFSET   0x100

#define TASK_KERNEL_RSP             (8 * 0)
#define TASK_KERNEL_STACK_BASE      (8 * 1)
#define TASK_KERNEL_STACK_LIMIT     (8 * 2)
#define TASK_KERNEL_FS              (8 * 3)
#define TASK_USER_RSP               (8 * 4)
#define TASK_USER_STACK_BASE        (8 * 5)
#define TASK_USER_STACK_LIMIT       (8 * 6)
#define TASK_USER_FS                (8 * 7)
#define TASK_USER_ENTRY_ADDR        (8 * 8)

/* arch_prctl syscall number and parameter */
#define ARCH_PRCTL                  0x9E
#define ARCH_SET_FS                 0x01002
#define ARCH_GET_FS                 0x01003

#endif /* __ASSEMBLY__ */

#endif /* __OCCLUM_TASK_H__ */
