#include <stdio.h>
#include <stdint.h>
#include "test.h"

// ============================================================================
// Helper functions for rdtsc
// ============================================================================

static inline uint64_t native_rdtsc() {
    uint64_t low, high;
    asm volatile("rdtsc" : "=a"(low), "=d"(high));
    return (high << 32) | low;
}

// ============================================================================
// Test cases for rdtsc
// ============================================================================

int test_rdtsc() {
    uint64_t start_count = native_rdtsc();
    if (start_count == 0) {
        THROW_ERROR("call rdtsc failed");
    }
    uint64_t end_count = native_rdtsc();
    if (end_count <= start_count) {
        THROW_ERROR("check rdtsc return value failed");
    }
    return 0;
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_rdtsc),
};

int main() {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
