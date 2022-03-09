#include <time.h>
#include <unistd.h>
#include <errno.h>
#include <assert.h>
#include "test.h"

// ============================================================================
// Helper macros
// ============================================================================

#define S           (1000 * 1000 * 1000)
#define MS          (1000 * 1000)
#define US          (1000)
#define NS          (1)

// ============================================================================
// Global variables
// ============================================================================

static const int SUCCESS = 1;
static const int FAIL = -1;

// ============================================================================
// Helper functions
// ============================================================================

static inline void validate_timespec(const struct timespec *tv) {
    assert(tv->tv_sec >= 0 && tv->tv_nsec >= 0 && tv->tv_nsec < S);
}


// retval = (a < b) ? -1 : ((a > b) ? 1 : 0)
static int timespec_cmp(const struct timespec *a, const struct timespec *b) {
    validate_timespec(a);
    validate_timespec(b);

    if (a->tv_sec < b->tv_sec) {
        return -1;
    } else if (a->tv_sec > b->tv_sec) {
        return 1;
    } else {
        return a->tv_nsec < b->tv_nsec ? -1 :
               (a->tv_nsec > b->tv_nsec ? 1 : 0);
    }
}

// diff = | a - b |
static void timespec_diff(const struct timespec *a, const struct timespec *b,
                          struct timespec *diff) {
    validate_timespec(a);
    validate_timespec(b);

    const struct timespec *begin, *end;
    if (timespec_cmp(a, b) <= 0) {
        begin = a;
        end = b;
    } else {
        begin = b;
        end = a;
    }

    diff->tv_nsec = end->tv_nsec - begin->tv_nsec;
    diff->tv_sec = end->tv_sec - begin->tv_sec;
    if (diff->tv_nsec < 0) {
        diff->tv_nsec += S;
        diff->tv_sec -= 1;
    }

    validate_timespec(diff);
}

// retval = | a - b | <= precision
static int timespec_equal(const struct timespec *a, const struct timespec *b,
                          const struct timespec *precision) {
    struct timespec diff;
    timespec_diff(a, b, &diff);
    if (timespec_cmp(&diff, precision) <= 0) {
        return 1;
    } else {
        printf("Greater than precision, diff={ %ld s, %ld ns}, precision={ %ld s, %ld ns}\n",
               diff.tv_sec, diff.tv_nsec, precision->tv_sec, precision->tv_nsec);
        return 0;
    }
}


// Return SUCCESS(1) if check passed, FAIL(-1) if check failed
static int check_nanosleep(const struct timespec *expected_sleep_period) {
    // The time obtained from Occlum is not very precise.
    // Here we take 1 millisecond as the time precision of Occlum.
    static struct timespec OS_TIME_PRECISION = {
        .tv_sec = 0,
        .tv_nsec = 1 * MS,
    };

    struct timespec begin_timestamp, end_timestamp;
    clock_gettime(CLOCK_MONOTONIC, &begin_timestamp);

    if (nanosleep(expected_sleep_period, NULL) != 0) {
        THROW_ERROR("nanosleep failed");
    }

    clock_gettime(CLOCK_MONOTONIC, &end_timestamp);
    struct timespec actual_sleep_period;
    timespec_diff(&begin_timestamp, &end_timestamp, &actual_sleep_period);

    return timespec_equal(expected_sleep_period, &actual_sleep_period,
                          &OS_TIME_PRECISION) ? SUCCESS : FAIL;
}

// ============================================================================
// Test cases
// Return SUCCESS(1) if check passed, FAIL(-1) if check failed
// ============================================================================

static int test_nanosleep_0_second() {
    struct timespec period_of_0s = { .tv_sec = 0, .tv_nsec = 0 };
    return check_nanosleep(&period_of_0s);
}

static int test_nanosleep_1_second() {
    struct timespec period_of_1s = { .tv_sec = 1, .tv_nsec = 0 };
    return check_nanosleep(&period_of_1s);
}

static int test_nanosleep_10ms() {
    struct timespec period_of_10ms = { .tv_sec = 0, .tv_nsec = 10 * MS };
    return check_nanosleep(&period_of_10ms);
}

// ============================================================================
// Test cases with invalid arguments
// Return SUCCESS(1) if check passed, FAIL(-1) if check failed
// ============================================================================

static int test_nanosleep_with_null_req() {
    if (nanosleep(NULL, NULL) != -1 && errno != EINVAL) {
        THROW_ERROR("nanosleep should report error");
    }
    return SUCCESS;
}

static int test_nanosleep_with_negative_tv_sec() {
    // nanosleep returns EINVAL if the value in the tv_sec field is negative
    struct timespec invalid_period = { .tv_sec = -1, .tv_nsec = 0};
    if (nanosleep(&invalid_period, NULL) != -1 && errno != EINVAL) {
        THROW_ERROR("nanosleep should report EINVAL error");
    }
    return SUCCESS;
}

static int test_nanosleep_with_negative_tv_nsec() {
    // nanosleep returns EINVAL if the value in the tv_nsec field
    // was not in the range 0 to 999999999.
    struct timespec invalid_period = { .tv_sec = 0, .tv_nsec = -1};
    if (nanosleep(&invalid_period, NULL) != -1 && errno != EINVAL) {
        THROW_ERROR("nanosleep should report EINVAL error");
    }
    return SUCCESS;
}

static int test_nanosleep_with_too_large_tv_nsec() {
    // nanosleep returns EINVAL if the value in the tv_nsec field
    // was not in the range 0 to 999999999 (10^6 - 1).
    struct timespec invalid_period = { .tv_sec = 0, .tv_nsec = S};
    if (nanosleep(&invalid_period, NULL) != -1 && errno != EINVAL) {
        THROW_ERROR("nanosleep should report EINVAL error");
    }
    return SUCCESS;
}

// ============================================================================
// Test suite main
// ============================================================================

// TODO: test interruption
static test_case_t test_cases[] = {
    TEST_CASE(test_nanosleep_0_second),
    TEST_CASE(test_nanosleep_1_second),
    TEST_CASE(test_nanosleep_10ms),
    TEST_CASE(test_nanosleep_with_null_req),
    TEST_CASE(test_nanosleep_with_negative_tv_sec),
    TEST_CASE(test_nanosleep_with_negative_tv_nsec),
    TEST_CASE(test_nanosleep_with_too_large_tv_nsec)
};

int main() {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
