#include <unistd.h>
#include <stdlib.h>
#include <sys/syscall.h>
#include <stdint.h>
#include <sys/mman.h>
#include "test.h"

// ============================================================================
// Helper function
// ============================================================================

#define PAGE_SIZE 4096

const static uint8_t magic_num_01 = 0xFF;
typedef struct syscall_args {
    int num;
    unsigned long arg0;
} syscall_args_t;

static inline uint64_t native_syscall(syscall_args_t *p) {
    uint64_t ret;
    register int num asm ("rax") = p->num;
    register unsigned long arg0 asm ("rdi") = p->arg0;

    asm volatile("syscall"
                 : "=a" (ret)
                 : "r" (num), "r" (arg0));
    return ret;
}

static uint64_t brk_syscall(uint64_t brk) {
    syscall_args_t brk_arg = {
        .num = __NR_brk,
        .arg0 = brk,
    };

    return native_syscall(&brk_arg);
}

// ============================================================================
// Test cases for access
// ============================================================================

static int test_brk_shrinks() {
    char *zero_buf = malloc(PAGE_SIZE * 2);
    if (zero_buf == NULL) {
        THROW_ERROR("malloc failed");
    }
    memset(zero_buf, 0, PAGE_SIZE * 2);

    uint64_t original_brk = brk_syscall(0);
    if (original_brk == 0) {
        THROW_ERROR("sbrk failed");
    }
    printf("original brk = %lx\n", original_brk);

    // increase brk
    printf("increase brk\n");
    uint64_t ret = brk_syscall(original_brk + PAGE_SIZE * 4);
    if (ret == 0) {
        THROW_ERROR("extend brk failed");
    }

    // set some values to the brk memory
    uint64_t test_range_start = original_brk + PAGE_SIZE * 2;
    for (int i = 0; i < PAGE_SIZE; i++) {
        *(int *)test_range_start = magic_num_01;
    }

    // decrease brk
    printf("decrease brk\n");
    ret = brk_syscall(original_brk + PAGE_SIZE * 2);
    if (ret != test_range_start) {
        THROW_ERROR("shrink brk failed");
    }
    printf("test range start = %lx\n", test_range_start);

    // increase brk
    uint64_t test_range_end = brk_syscall(original_brk + PAGE_SIZE * 4);
    if (test_range_end != original_brk + PAGE_SIZE * 4) {
        THROW_ERROR("extend brk failed");
    }

    if ( memcmp((const void *)test_range_start, zero_buf, PAGE_SIZE * 2) != 0) {
        THROW_ERROR("sbrk not reset memory");
    }

    free(zero_buf);

    return 0;
}

#ifdef SGX_MODE_HW
// This test case will fail in simulation mode. Because the raw syscall interface are not handled by Occlum
// in simulation mode.
//
// Use brk to allocate 4 pages and test brk and mprotect
//    original brk
//       | page 00          page 02
//       |         page 01          page 03
// ...---|-------|-------|-------|-------|
static int test_brk_shrinks_spans_multiple_chunks() {
    const static uint8_t magic_num_02 = 0xFE;
    char *zero_buf = malloc(PAGE_SIZE * 4);
    if (zero_buf == NULL) {
        THROW_ERROR("malloc failed");
    }
    memset(zero_buf, 0, PAGE_SIZE * 4);

    size_t original_brk = brk_syscall(0);
    if (original_brk == 0) {
        THROW_ERROR("brk failed");
    }
    printf("original brk = %lx\n", original_brk);

    // increase brk to the end of page 03
    size_t ret = brk_syscall(original_brk + PAGE_SIZE * 4);
    if (ret != original_brk + PAGE_SIZE * 4) {
        THROW_ERROR("extend brk failed");
    }

    // set some values to the brk memory page 02
    size_t test_range_start = original_brk + PAGE_SIZE * 2;
    for (int i = 0; i < PAGE_SIZE; i++) {
        *(int *)test_range_start = magic_num_01;
    }

    // mprotect page 01 - 03 to PROT_NONE and decrease brk to the end of page 00
    int rc = mprotect((void *)(original_brk + PAGE_SIZE * 1), PAGE_SIZE * 3, PROT_NONE);
    if (rc < 0) {
        THROW_ERROR("mprotect failure");
    }
    ret = brk_syscall(original_brk + PAGE_SIZE * 1);
    if (ret != original_brk + PAGE_SIZE * 1) {
        THROW_ERROR("shrink brk failed");
    }

    // increase brk to the end of page 02
    ret = brk_syscall(original_brk + PAGE_SIZE * 3);
    if (ret != original_brk + PAGE_SIZE * 3) {
        THROW_ERROR("extend brk failed");
    }

    // set some values to the brk memory page 01
    test_range_start = original_brk + PAGE_SIZE * 1;
    for (int i = 0; i < PAGE_SIZE; i++) {
        *(int *)test_range_start = magic_num_02;
    }

    // decrease brk again to the end of page 00
    rc = mprotect((void *)(original_brk + PAGE_SIZE * 1), PAGE_SIZE * 2, PROT_NONE);
    if (rc < 0) {
        THROW_ERROR("mprotect failure");
    }
    ret = brk_syscall(original_brk + PAGE_SIZE * 1);
    if (ret != original_brk + PAGE_SIZE * 1) {
        THROW_ERROR("shrink brk failed");
    }

    // increase brk to the end of page 03
    ret = brk_syscall(original_brk + PAGE_SIZE * 4);
    if (ret != original_brk + PAGE_SIZE * 4) {
        THROW_ERROR("extend brk failed");
    }

    if ( memcmp((const void *)original_brk, zero_buf, PAGE_SIZE * 4) != 0) {
        THROW_ERROR("brk not reset memory");
    }

    // decrease brk to the original brk
    ret = brk_syscall(original_brk);
    if (ret != original_brk) {
        THROW_ERROR("shrink brk failed");
    }

    free(zero_buf);

    return 0;
}
#endif

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_brk_shrinks),
#ifdef SGX_MODE_HW
    TEST_CASE(test_brk_shrinks_spans_multiple_chunks),
#endif
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
