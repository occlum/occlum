#ifndef __SWITCH_H__
#define __SWITCH_H__

#ifdef __ASSEMBLY__

/*In SGX SDK the GS register point to thread_data_t structure and a whole page is
assigned to the structure. So any offset larger than sizeof(thread_data_t) and
less than 4096 is unused by anyone. We can use it.*/
#define TD_USER_RIP         (0x100)
#define TD_USER_CONTEXT     (0x108)
#define TD_KERNEL_RSP       (0x110)
#define TD_KERNEL_FS        (0x118)
#define TD_KERNEL_JMPBUF    (0x120)
#define TD_USER_FAULT       (0x128)

#define CPU_CONTEXT_R8      (0*8)
#define CPU_CONTEXT_R9      (1*8)
#define CPU_CONTEXT_R10     (2*8)
#define CPU_CONTEXT_R11     (3*8)
#define CPU_CONTEXT_R12     (4*8)
#define CPU_CONTEXT_R13     (5*8)
#define CPU_CONTEXT_R14     (6*8)
#define CPU_CONTEXT_R15     (7*8)
#define CPU_CONTEXT_RDI     (8*8)
#define CPU_CONTEXT_RSI     (9*8)
#define CPU_CONTEXT_RBP     (10*8)
#define CPU_CONTEXT_RBX     (11*8)
#define CPU_CONTEXT_RDX     (12*8)
#define CPU_CONTEXT_RAX     (13*8)
#define CPU_CONTEXT_RCX     (14*8)
#define CPU_CONTEXT_RSP     (15*8)
#define CPU_CONTEXT_RIP     (16*8)
#define CPU_CONTEXT_RFLAGS  (17*8)
#define CPU_CONTEXT_FSBASE  (18*8)

/* arch_prctl syscall number and parameter */
#define ARCH_PRCTL          (0x9E)
#define ARCH_SET_FS         (0x01002)
#define ARCH_GET_FS         (0x01003)

#else /* ! __ASSEMBLY_ */

#include <stddef.h>
#include <sys/types.h>
#include <setjmp.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    uint64_t r8;
    uint64_t r9;
    uint64_t r10;
    uint64_t r11;
    uint64_t r12;
    uint64_t r13;
    uint64_t r14;
    uint64_t r15;
    uint64_t rdi;
    uint64_t rsi;
    uint64_t rbp;
    uint64_t rbx;
    uint64_t rdx;
    uint64_t rax;
    uint64_t rcx;
    uint64_t rsp;
    uint64_t rip;
    uint64_t rflags;
    uint64_t fsbase;
    void* fpregs;
} CpuContext;

void switch_to_user(CpuContext* user_context);

#ifdef __cplusplus
}
#endif

#endif /* __ASSEMBLY__ */

#endif /* __SWITCH_H__ */
