#include <sys/time.h>
#include <time.h>
#include "test.h"

// ============================================================================
// Test cases for gettimeofday
// ============================================================================

int test_gettimeofday() {
    struct timeval tv;
    if (gettimeofday(&tv, NULL)) {
        throw_error("gettimeofday failed");
    }
    return 0;
}

// ============================================================================
// Test cases for clock_gettime
// ============================================================================

int test_clock_gettime() {
    struct timespec ts;
    if (clock_gettime(CLOCK_REALTIME, &ts)) {
        throw_error("clock_gettime(CLOCK_REALTIME, ...) failed");
    }
    if (clock_gettime(CLOCK_MONOTONIC, &ts)) {
        throw_error("clock_gettime(CLOCK_MONOTONIC, ...) failed");
    }
    return 0;
}

// ============================================================================
// Test suite
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_gettimeofday),
    TEST_CASE(test_clock_gettime),
};

int main() {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
