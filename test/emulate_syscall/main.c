#include <sys/mman.h>
#include <sys/syscall.h>
#include <stdio.h>
#include <stdint.h>
#include "test.h"

// ============================================================================
// Helper structs & functions
// ============================================================================

typedef struct syscall_args {
    int num;
    unsigned long arg0;
    unsigned long arg1;
    unsigned long arg2;
    unsigned long arg3;
    unsigned long arg4;
    unsigned long arg5;
} syscall_args_t;

static inline uint64_t native_syscall(syscall_args_t *p) {
    uint64_t ret;
    register int num asm ("rax") = p->num;
    register unsigned long arg0 asm ("rdi") = p->arg0;
    register unsigned long arg1 asm ("rsi") = p->arg1;
    register unsigned long arg2 asm ("rdx") = p->arg2;
    register unsigned long arg3 asm ("r10") = p->arg3;
    register unsigned long arg4 asm ("r8") = p->arg4;
    register unsigned long arg5 asm ("r9") = p->arg5;

    asm volatile("syscall"
                 : "=a" (ret)
                 : "r" (num), "r" (arg0), "r" (arg1), "r" (arg2), "r" (arg3), "r" (arg4), "r" (arg5));
    return ret;
}

// ============================================================================
// Test cases for syscall emulation
// ============================================================================

#define KB                      (1024UL)
#define PAGE_SIZE               (4 * KB)

/*
 * We use mmap() to test because it employs all arguments.
 */
int test_mmap_and_munmap_via_syscall_instruction() {
    int len = PAGE_SIZE;
    syscall_args_t mmap_arg = {
        .num = __NR_mmap,
        .arg0 = (unsigned long) NULL,
        .arg1 = len,
        .arg2 = PROT_READ | PROT_WRITE,
        .arg3 = MAP_PRIVATE | MAP_ANONYMOUS,
        .arg4 = -1,
        .arg5 = 0,
    };
    char *buf = (char *) native_syscall(&mmap_arg);
    if (buf == MAP_FAILED) {
        THROW_ERROR("syscall mmap failed");
    }
    for (size_t bi = 0; bi < len; bi++) {
        if (buf[bi] != '\0') {
            THROW_ERROR("invalid buffer contents");
        }
    }

    syscall_args_t munmap_arg = {
        .num = __NR_munmap,
        .arg0 = (unsigned long) buf,
        .arg1 = len,
    };
    int ret = native_syscall(&munmap_arg);
    if (ret < 0) {
        THROW_ERROR("syscall munmap failed");
    }
    return 0;
}

// ============================================================================
// Test suite main
// ============================================================================
static test_case_t test_cases[] = {
    TEST_CASE(test_mmap_and_munmap_via_syscall_instruction),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
