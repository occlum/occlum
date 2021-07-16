#include <stdio.h>
#include <sys/timerfd.h>
#include <sys/select.h>
#include <time.h>
#include "test.h"

// ============================================================================
// Test cases for timerfd full process
// ============================================================================
int test_timerfd() {
    int tfd = timerfd_create(CLOCK_REALTIME,  0);

    printf("Starting at (%d)...\n", (int)time(NULL));
    if (tfd <= 0) {
        THROW_ERROR("timerfd_create(CLOCK_REALTIME, ...) failed");
    }
    char dummybuf[8];
    struct itimerspec spec = {
        { 1, 0 }, // Set to {0, 0} if you need a one-shot timer
        { 2, 0 }
    };
    struct itimerspec curr = {
        { 0, 0 }, // Set to {0, 0} if you need a one-shot timer
        { 0, 0 }
    };
    if (timerfd_settime(tfd, 0, &spec, NULL)) {
        THROW_ERROR("timerfd_settime(...) failed");
    }
    if (timerfd_gettime(tfd, &curr)) {
        THROW_ERROR("timerfd_gettime(...) failed");
    }
    /* Wait */
    fd_set rfds;
    int retval;

    /* Watch timefd file descriptor */
    FD_ZERO(&rfds);
    FD_SET(0, &rfds);
    FD_SET(tfd, &rfds);

    /* Let's wait for initial timer expiration */
    retval = select(tfd + 1, &rfds, NULL, NULL,
                    NULL); /* Last parameter = NULL --> wait forever */
    printf("Expired at %d! (%d) (%ld)\n", (int)time(NULL), retval, read(tfd, dummybuf, 8) );

    /* Let's wait for initial timer expiration */
    retval = select(tfd + 1, &rfds, NULL, NULL, NULL);
    printf("Expired at %d! (%d) (%ld)\n", (int)time(NULL), retval, read(tfd, dummybuf, 8) );

    retval = select(tfd + 1, &rfds, NULL, NULL, NULL);
    printf("Expired at %d! (%d) (%ld)\n", (int)time(NULL), retval, read(tfd, dummybuf, 8) );
    return 0;
}

int test_invalid_argument() {
    int tfd = timerfd_create(CLOCK_REALTIME,  0);
    if (tfd <= 0) {
        THROW_ERROR("timerfd_create(CLOCK_REALTIME, ...) failed");
    }
    int invalid_clockid = 6;
    int invalid_create_flags = 11;
    int invalid_settime_flags = 5;
    struct itimerspec spec = {
        { 1, 0 }, // Set to {0, 0} if you need a one-shot timer
        { 2, 0 }
    };
    /* Test invalid argument */
    int ret = timerfd_create(CLOCK_REALTIME, invalid_create_flags);
    if (ret >= 0 || errno != EINVAL ) {
        THROW_ERROR("failed to check timerfd_create with invalid flags");
    }
    ret = timerfd_create(invalid_clockid,  0);
    if (ret >= 0 || errno != EINVAL ) {
        THROW_ERROR("failed to check timerfd_create with invalid clockid");
    }
    ret = timerfd_settime(tfd, invalid_settime_flags, &spec, NULL);
    if (ret >= 0 || errno != EINVAL ) {
        THROW_ERROR("failed to check timerfd_settime with invalid flags");
    }
    return 0;
}

// ============================================================================
// Test suite
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_timerfd),
    TEST_CASE(test_invalid_argument),
};

int main() {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}

