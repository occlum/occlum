#include <sys/epoll.h>
#include <sys/eventfd.h>
#include <sys/select.h>
#include <sys/socket.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <fcntl.h>
#include <unistd.h>
#include <poll.h>
#include <pthread.h>
#include <stdlib.h>
#include <stdio.h>
#include <spawn.h>
#include <string.h>
#include <errno.h>
#include <stdarg.h>
#include <pthread.h>
#include "test.h"

// ============================================================================
// Helper definition
// ============================================================================

#define MAXEVENTS 64
#define TEST_DATA 678

struct thread_arg {
    pthread_t tid;
    int fd;
    uint64_t data;
};

// ============================================================================
// Helper functions
// ============================================================================

static void *thread_child(void *arg) {
    struct thread_arg *child_arg = arg;

    printf("epoll_wait 1...\n");
    struct epoll_event events[MAXEVENTS] = {0};
    int nfds = epoll_wait(child_arg->fd, events, MAXEVENTS, -1);
    if (nfds < 0) {
        return (void *) -1;
    }
    printf("epoll_wait 1 success.\n");

    sleep(1);

    printf("epoll_wait 2...\n");
    nfds = epoll_wait(child_arg->fd, events, MAXEVENTS, -1);
    if (nfds < 0) {
        return (void *) -1;
    }
    printf("epoll_wait 2 success.\n");
    return NULL;
}

int create_child(struct thread_arg *arg) {
    pthread_attr_t attr;
    if (pthread_attr_init(&attr) != 0) {
        THROW_ERROR("failed to initialize attribute");
    }

    if (pthread_create(&(arg->tid), &attr, &thread_child, arg) != 0) {
        if (pthread_attr_destroy(&attr) != 0) {
            THROW_ERROR("failed to destroy attr");
        }
        THROW_ERROR("failed to create the thread");
    }

    if (pthread_attr_destroy(&attr) != 0) {
        THROW_ERROR("failed to destroy attr");
    }

    return 0;
}

// This test intends to test that the epoll_wait can be waken epoll_ctl
int test_epoll_ctl_main(int end_fd_1, int end_fd_2) {
    uint64_t data = TEST_DATA;
    struct thread_arg child_arg;

    int epfd = epoll_create1(0);
    if (epfd == -1) {
        THROW_ERROR("epoll_create failed");
    }

    // watch for end_fd_1
    struct epoll_event event;
    event.data.fd = end_fd_1;
    event.events = EPOLLIN | EPOLLET;
    int ret = epoll_ctl(epfd, EPOLL_CTL_ADD, end_fd_1, &event);
    if (ret == -1) {
        close(epfd);
        THROW_ERROR("epoll_ctl add failed");
    }

    // write to end_fd_2
    int write_size = write(end_fd_2, &data, sizeof(data));
    if (write_size < 0) {
        THROW_ERROR("failed to write an eventfd");
    }

    child_arg.data = 0;
    child_arg.fd = epfd;
    child_arg.tid = 0;
    if (create_child(&child_arg) != 0) {
        close(epfd);
        THROW_ERROR("failed to create children");
    }

    // wait for child thread to start second time epoll_wait
    sleep(3);

    printf("second time epoll ctl\n");
    ret = epoll_ctl(epfd, EPOLL_CTL_MOD, end_fd_1, &event);
    if (ret == -1) {
        close(epfd);
        THROW_ERROR("epoll_ctl mod failed");
    }

    pthread_join(child_arg.tid, NULL);
    close(epfd);

    return 0;
}

// ============================================================================
// Test cases for anonymous mmap
// ============================================================================

int test_epoll_ctl_uds() {
    int sockets[2];

    if (socketpair(AF_UNIX, SOCK_STREAM, 0, sockets) < 0) {
        THROW_ERROR("opening stream socket pair");
    }

    int ret = test_epoll_ctl_main(sockets[0], sockets[1]);
    close(sockets[0]);
    close(sockets[1]);

    if (ret < 0) {
        THROW_ERROR("epoll ctl test eventfd failure");
    }

    return 0;
}

int test_epoll_ctl_eventfd() {
    int event_fd = eventfd(0, EFD_NONBLOCK);
    if (event_fd < 0) {
        THROW_ERROR("failed to create an eventfd");
    }

    int ret = test_epoll_ctl_main(event_fd, event_fd);
    close(event_fd);

    if (ret < 0) {
        THROW_ERROR("epoll ctl test eventfd failure");
    }
    return 0;
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_epoll_ctl_eventfd),
    TEST_CASE(test_epoll_ctl_uds),
};

int main() {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
