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

#include "test.h"

#define MAXEVENTS 1

// ============================================================================
// Test cases
// ============================================================================

int test_fcntl_get_flags() {
    int event_fd = eventfd(0, 0);
    if (event_fd < 0) {
        THROW_ERROR("failed to create an eventfd");
    }

    if ((fcntl(event_fd, F_GETFL, 0) != O_RDWR)) {
        close(event_fd);
        THROW_ERROR("fcntl get flags failed");
    }

    close(event_fd);
    return 0;
}

int test_fcntl_set_flags() {
    int event_fd = eventfd(0, 0);
    if (event_fd < 0) {
        THROW_ERROR("failed to create an eventfd");
    }

    fcntl(event_fd, F_SETFL, O_NONBLOCK);
    if ((fcntl(event_fd, F_GETFL, 0) != (O_NONBLOCK | O_RDWR))) {
        close(event_fd);
        THROW_ERROR("fcntl set flags failed");
    }

    close(event_fd);
    return 0;
}

int test_create_with_flags() {
    int event_fd = eventfd(0, EFD_NONBLOCK);
    if (event_fd < 0) {
        THROW_ERROR("failed to create an eventfd");
    }

    if ((fcntl(event_fd, F_GETFL, 0) != (O_NONBLOCK | O_RDWR))) {
        close(event_fd);
        THROW_ERROR("create flags failed\n");
    }

    close(event_fd);
    return 0;
}

struct thread_arg {
    pthread_t tid;
    int fd;
    uint64_t data;
};

#define TEST_DATA 678
#define CHILD_NUM 16

static void *thread_child(void *arg) {
    struct thread_arg *child_arg = arg;
    write(child_arg->fd, &(child_arg->data), sizeof(child_arg->data));
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

int test_read_write() {
    int event_fd = eventfd(0, 0);
    if (event_fd < 0) {
        THROW_ERROR("failed to create an eventfd");
    }

    struct thread_arg child_arg[CHILD_NUM] = {0};

    // Create child threads and send eventfd and data
    for (int i = 0; i < CHILD_NUM; i++) {
        child_arg[i].fd = event_fd;
        child_arg[i].data = TEST_DATA;
        if (create_child(&child_arg[i]) != 0) {
            close(event_fd);
            THROW_ERROR("failed to create children");
        }
    }

    // Check the data sent from children
    uint64_t data_recv = 0;

    do {
        uint64_t cur_data = 0;
        ssize_t len_recv = read(event_fd, &cur_data, sizeof(uint64_t));
        if (len_recv != sizeof(uint64_t)) {
            close(event_fd);
            THROW_ERROR("received length is not as expected");
        }
        data_recv += cur_data;
    } while (data_recv != TEST_DATA * CHILD_NUM);

    close(event_fd);

    for (int i = 0; i < CHILD_NUM; i++) {
        if (pthread_join(child_arg[i].tid, NULL) != 0) {
            THROW_ERROR("pthread_join");
        }
    }

    return 0;
}

int test_select_with_socket() {
    fd_set rfds, wfds;
    int ret = 0;

    struct timeval tv = { .tv_sec = 60, .tv_usec = 0 };

    int sock = socket(AF_INET, SOCK_STREAM, 0);
    int event_fd = eventfd(0, 0);
    if (event_fd < 0 || sock < 0) {
        THROW_ERROR("failed to create files");
    }

    FD_ZERO(&rfds);
    FD_ZERO(&wfds);
    FD_SET(sock, &rfds);
    FD_SET(sock, &wfds);
    FD_SET(event_fd, &rfds);
    FD_SET(event_fd, &wfds);
    ret = select(sock > event_fd ? sock + 1 : event_fd + 1, &rfds, &wfds, NULL, &tv);
    if (ret != 3) {
        close_files(2, sock, event_fd);
        THROW_ERROR("select failed");
    }

    if (FD_ISSET(event_fd, &rfds) == 1 || FD_ISSET(event_fd, &wfds) == 0 ||
            FD_ISSET(sock, &rfds) == 0 || FD_ISSET(sock, &wfds) == 0) {
        close_files(2, sock, event_fd);
        THROW_ERROR("bad select return");
    }

    close_files(2, sock, event_fd);
    return 0;
}

int test_poll_with_socket() {
    int sock = socket(AF_INET, SOCK_STREAM, 0);
    int event_fd = eventfd(0, 0);
    if (event_fd < 0 || sock < 0) {
        THROW_ERROR("failed to create files");
    }

    struct pollfd pollfds[] = {
        { .fd = sock, .events = POLLIN, .revents = 0, },
        { .fd = event_fd, .events = POLLOUT, .revents = 0 },
    };

    int ret = poll(pollfds, 2, -1);
    if (ret <= 0) {
        close_files(2, event_fd, sock);
        THROW_ERROR("poll error");
    }

    close_files(2, event_fd, sock);
    return 0;
}

int test_epoll_with_socket() {
    int event_fd = eventfd(0, EFD_NONBLOCK);
    int sock = socket(AF_INET, SOCK_STREAM, 0);
    int epfd = epoll_create1(0);

    if (event_fd < 0 || sock < 0 || epfd < 0) {
        THROW_ERROR("failed to create files");
    }

    struct epoll_event ctl_events[2] = {0};
    // Add eventfd to the interest list
    ctl_events[0].data.fd = event_fd;
    ctl_events[0].events = EPOLLIN | EPOLLET;
    // Add socket to the interest list
    ctl_events[1].data.fd = sock;
    ctl_events[1].events = EPOLLIN | EPOLLET;
    if (epoll_ctl(epfd, EPOLL_CTL_ADD, event_fd, &ctl_events[0]) == -1 ||
            epoll_ctl(epfd, EPOLL_CTL_ADD, sock, &ctl_events[1]) == -1) {
        close_files(3, event_fd, sock, epfd);
        THROW_ERROR("epoll_ctl");
    }

    struct thread_arg child_arg = { .tid = 0, .fd = event_fd, .data = TEST_DATA };
    if (create_child(&child_arg) != 0) {
        close_files(3, event_fd, sock, epfd);
        THROW_ERROR("failed to create child");
    }

    struct epoll_event events[MAXEVENTS] = {0};
    if (epoll_pwait(epfd, events, MAXEVENTS, -1, NULL) <= 0) {
        close_files(3, event_fd, sock, epfd);
        THROW_ERROR("epoll failed");
    }

    close_files(3, event_fd, sock, epfd);

    if (pthread_join(child_arg.tid, NULL) != 0) {
        THROW_ERROR("pthread_join");
    }

    return 0;
}
// ============================================================================
// Test suite
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_fcntl_get_flags),
    TEST_CASE(test_fcntl_set_flags),
    TEST_CASE(test_create_with_flags),
    TEST_CASE(test_read_write),
    TEST_CASE(test_epoll_with_socket),
    TEST_CASE(test_poll_with_socket),
    TEST_CASE(test_select_with_socket),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
