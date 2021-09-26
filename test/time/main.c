#include <sys/time.h>
#include <time.h>
#include "test.h"

// ============================================================================
// Test cases for gettimeofday
// ============================================================================

int test_gettimeofday() {
    struct timeval tv;
    if (gettimeofday(&tv, NULL)) {
        THROW_ERROR("gettimeofday failed");
    }
    return 0;
}

// ============================================================================
// Test cases for clock_gettime
// ============================================================================

int test_clock_gettime() {
    struct timespec ts;
    if (clock_gettime(CLOCK_REALTIME, &ts)) {
        THROW_ERROR("clock_gettime(CLOCK_REALTIME, ...) failed");
    }
    if (clock_gettime(CLOCK_MONOTONIC, &ts)) {
        THROW_ERROR("clock_gettime(CLOCK_MONOTONIC, ...) failed");
    }
    if (clock_gettime(CLOCK_MONOTONIC_RAW, &ts)) {
        THROW_ERROR("clock_gettime(CLOCK_MONOTONIC_RAW, ...) failed");
    }
    if (clock_gettime(CLOCK_REALTIME_COARSE, &ts)) {
        THROW_ERROR("clock_gettime(CLOCK_REALTIME_COARSE, ...) failed");
    }
    if (clock_gettime(CLOCK_MONOTONIC_COARSE, &ts)) {
        THROW_ERROR("clock_gettime(CLOCK_MONOTONIC_COARSE, ...) failed");
    }
    if (clock_gettime(CLOCK_BOOTTIME, &ts)) {
        THROW_ERROR("clock_gettime(CLOCK_BOOTTIME, ...) failed");
    }
    return 0;
}

// ============================================================================
// Test cases for clock_getres
// ============================================================================

int test_clock_getres() {
    struct timespec res;
    if (clock_getres(CLOCK_REALTIME, &res)) {
        THROW_ERROR("clock_getres(CLOCK_REALTIME, ...) failed");
    }
    if (clock_getres(CLOCK_MONOTONIC, &res)) {
        THROW_ERROR("clock_getres(CLOCK_MONOTONIC, ...) failed");
    }
    if (clock_getres(CLOCK_MONOTONIC_RAW, &res)) {
        THROW_ERROR("clock_getres(CLOCK_MONOTONIC_RAW, ...) failed");
    }
    if (clock_getres(CLOCK_REALTIME_COARSE, &res)) {
        THROW_ERROR("clock_getres(CLOCK_REALTIME_COARSE, ...) failed");
    }
    if (clock_getres(CLOCK_MONOTONIC_COARSE, &res)) {
        THROW_ERROR("clock_getres(CLOCK_MONOTONIC_COARSE, ...) failed");
    }
    if (clock_getres(CLOCK_BOOTTIME, &res)) {
        THROW_ERROR("clock_getres(CLOCK_BOOTTIME, ...) failed");
    }

    if (clock_getres(CLOCK_REALTIME, NULL)) {
        THROW_ERROR("clock_getres(CLOCK_REALTIME, NULL) failed");
    }
    return 0;
}

// ============================================================================
// Test suite
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_gettimeofday),
    TEST_CASE(test_clock_gettime),
    TEST_CASE(test_clock_getres),
};

int main() {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
