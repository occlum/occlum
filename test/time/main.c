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
    if (clock_getres(CLOCK_MONOTONIC_COARSE, &res)) {
        THROW_ERROR("clock_getres(CLOCK_MONOTONIC_COARSE, ...) failed");
    }
    if (clock_getres(CLOCK_REALTIME, NULL)) {
        THROW_ERROR("clock_getres(CLOCK_REALTIME, NULL) failed");
    }
    return 0;
}

// ============================================================================
// Test cases for localtime
// ============================================================================

int test_get_localtime() {
    time_t t = time(NULL);
    if (t == (time_t) -1) {
        THROW_ERROR("failed to get time");
    }
    struct tm *local_time = localtime(&t);
    if (local_time == NULL) {
        THROW_ERROR("failed to convert a time value to a local time");
    }
    printf("Offset to GMT is %lds.\n", local_time->tm_gmtoff);
    printf("The time zone is '%s'.\n", local_time->tm_zone);
    return 0;
}

// ============================================================================
// Test suite
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_gettimeofday),
    TEST_CASE(test_clock_gettime),
    TEST_CASE(test_clock_getres),
    TEST_CASE(test_get_localtime),
};

int main() {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
