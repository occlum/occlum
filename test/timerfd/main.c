#include <stdio.h>
#include <sys/epoll.h>
#include <sys/timerfd.h>
#include <sys/select.h>
#include <pthread.h>
#include <fcntl.h>
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
    struct timeval timeout;

    /* Watch timefd file descriptor */
    FD_ZERO(&rfds);
    FD_SET(tfd, &rfds);

    printf("it_value = %ld seconds, it_interval = %ld seconds\n",
           spec.it_value.tv_sec, spec.it_interval.tv_sec);

    /* Let's wait for initial timer expiration */
    retval = select(tfd + 1, &rfds, NULL, NULL,
                    NULL); /* Last parameter = NULL --> wait forever */
    printf("Expired at %d! (%d) (%ld)\n", (int)time(NULL), retval, read(tfd, dummybuf, 8) );

    /* Wait up to five seconds. */
    timeout.tv_sec = 5;
    timeout.tv_usec = 0;
    retval = select(tfd + 1, &rfds, NULL, NULL, &timeout);
    printf("Expired at %d! (%d) (%ld)\n", (int)time(NULL), retval, read(tfd, dummybuf, 8) );

    /* Wait up to 0.5 second. */
    timeout.tv_sec = 0;
    timeout.tv_usec = 500000;
    retval = select(tfd + 1, &rfds, NULL, NULL, &timeout);
    if (timerfd_gettime(tfd, &curr)) {
        THROW_ERROR("timerfd_gettime(...) failed");
    }
    printf("%ld ns left for next expire\n", curr.it_value.tv_nsec);
    printf("Expired at %d! (%d) (%ld)\n", (int)time(NULL), retval, read(tfd, dummybuf, 8) );

    printf("Set timerfd as non block mode\n");
    retval = fcntl(tfd, F_SETFL, TFD_NONBLOCK);
    if (retval == -1) {
        printf("fcntl failed\n");
    }

    printf("Disalarm timerfd\n");
    struct itimerspec stop = {
        { 0, 0 },
        { 0, 0 }
    };

    if (timerfd_settime(tfd, 0, &stop, NULL)) {
        THROW_ERROR("timerfd_settime(...) failed");
    }

    int ret = read(tfd, dummybuf, 8);
    if (ret != -1) {
        THROW_ERROR("Expected return (-1) but actually it is %d\n", ret);
    }

    return 0;
}

int test_invalid_argument() {
    int tfd = timerfd_create(CLOCK_REALTIME, TFD_NONBLOCK);
    if (tfd <= 0) {
        THROW_ERROR("timerfd_create(CLOCK_REALTIME, ...) failed");
    }

    char dummybuf[8];
    int ret = read(tfd, dummybuf, 8);
    if (ret >= 0 || errno != EAGAIN ) {
        THROW_ERROR("failed to check reading disarmed timer");
    }

    int invalid_clockid = 6;
    int invalid_create_flags = 11;
    int invalid_settime_flags = 5;
    struct itimerspec spec = {
        { 1, 0 }, // Set to {0, 0} if you need a one-shot timer
        { 2, 0 }
    };
    /* Test invalid argument */
    ret = timerfd_create(CLOCK_REALTIME, invalid_create_flags);
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

// epoll test example

#define MX_EVNTS 10
#define EPL_TOUT 8000
#define MX_CNT 5

struct epoll_param {
    struct itimerspec its;
    int tfd;
};

static void *tfd_wait_func(void *arg) {
    struct epoll_event evnts[MX_EVNTS];
    int *eplfd = (int *)arg;
    int n = -1;
    size_t i, cnt = 0;

    printf("\nepoll wait start at %d\n", (int)time(NULL));

    while (1) {
        n = epoll_wait(*eplfd, evnts, MX_EVNTS, EPL_TOUT);
        if (n == -1) {
            perror("epoll_wait() error");
            break;
        } else if (n == 0) {
            printf("time out %d sec expired\n", EPL_TOUT / 1000);
            break;
        }

        printf("%d events received\n", n);
        for (i = 0; i < n; i++) {
            struct epoll_param *pm = (struct epoll_param *)(evnts[i].data.ptr);
            printf("tfd: %d current: %d, \tit_value: %ld, interval: %ld\n\n",
                   pm->tfd, (int)time(NULL),
                   (long)(pm->its.it_value.tv_sec),
                   (long)(pm->its.it_interval.tv_sec));

            /*handle timerFd*/
            uint64_t tmpExp = 0;
            read(pm->tfd, &tmpExp, sizeof(uint64_t));
        }

        if (++cnt == MX_CNT) {
            printf("cnt reached MX_CNT, %d\n", MX_CNT);
            break;
        }
    }

    pthread_exit(NULL);
}

static int create_timerfd_epoll(int eplfd, struct epoll_param *pm,
                                struct itimerspec *its) {
    int tfd = timerfd_create(CLOCK_REALTIME,  0);
    if (tfd < 0) {
        THROW_ERROR("timerfd_create failed");
    }

    if (timerfd_settime(tfd, 0, its, NULL)) {
        THROW_ERROR("timerfd_settime failed");
    }

    /* add timerfd to epoll */
    pm->its = *its;
    pm->tfd = tfd;
    struct epoll_event ev;
    ev.events = EPOLLIN | EPOLLET;
    ev.data.ptr = pm;
    if (epoll_ctl(eplfd, EPOLL_CTL_ADD, tfd, &ev) != 0) {
        THROW_ERROR("epoll_ctl() error");
    }

    return 0;
}

int test_with_epoll() {
    int ret;
    int eplfd = epoll_create1(0);
    if (eplfd < 0) {
        THROW_ERROR("epoll_create1() error");
    }

    /* Create first timer fd */
    struct itimerspec its = {
        { 1, 0 },
        { 3, 0 }, // Set to {0, 0} if you need a one-shot timer
    };

    struct epoll_param pm;
    ret = create_timerfd_epoll(eplfd, &pm, &its);
    if (ret < 0) {
        return -1;
    }

    /* Create second timer fd */
    struct itimerspec its2 = {
        { 1, 0 }, // Set to {0, 0} if you need a one-shot timer
        { 2, 0 }
    };

    struct epoll_param pm2;
    ret = create_timerfd_epoll(eplfd, &pm2, &its2);
    if (ret < 0) {
        return -1;
    }

    pthread_t pid;
    if (pthread_create(&pid, NULL, tfd_wait_func, (void *)&eplfd) != 0) {
        perror("pthread_create() error");
        return -1;
    }

    if (pthread_join(pid, NULL) != 0) {
        THROW_ERROR("pthread_join() error");
        return -1;
    }
    close(pm.tfd);
    close(pm2.tfd);
    close(eplfd);
    return 0;
}

// ============================================================================
// Test suite
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_timerfd),
    TEST_CASE(test_invalid_argument),
    TEST_CASE(test_with_epoll),
};

int main() {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}

