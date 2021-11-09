#define _GNU_SOURCE
#include <errno.h>
#include <sys/epoll.h>
#include <sys/select.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <sys/time.h>
#include <sys/stat.h>
#include <sys/ioctl.h>
#include <fcntl.h>
#include <poll.h>
#include <unistd.h>
#include <stdlib.h>
#include <stdio.h>
#include <spawn.h>
#include <string.h>

#include "test.h"

// ============================================================================
// Helper function
// ============================================================================
static void free_pipe(int *pipe) {
    close(pipe[0]);
    close(pipe[1]);
}

// ============================================================================
// Test cases
// ============================================================================
int test_fstat() {
    int pipe_fds[2];
    struct stat stat_bufs[2];
    if (pipe(pipe_fds) < 0) {
        THROW_ERROR("failed to create a pipe");
    }
    if (fstat(pipe_fds[0], &stat_bufs[0]) < 0 || fstat(pipe_fds[1], &stat_bufs[1]) < 0) {
        free_pipe(pipe_fds);
        THROW_ERROR("failed to fstat pipe fd");
    }
    free_pipe(pipe_fds);
    if (!S_ISFIFO(stat_bufs[0].st_mode) || !S_ISFIFO(stat_bufs[1].st_mode)) {
        THROW_ERROR("failed to check the pipe st_mode");
    }
    return 0;
}

int test_fcntl_get_flags() {
    int pipe_fds[2];
    if (pipe(pipe_fds) < 0) {
        THROW_ERROR("failed to create a pipe");
    }

    if ((fcntl(pipe_fds[0], F_GETFL, 0) != O_RDONLY) ||
            (fcntl(pipe_fds[1], F_GETFL, 0) != O_WRONLY)) {
        free_pipe(pipe_fds);
        THROW_ERROR("fcntl get flags failed");
    }

    free_pipe(pipe_fds);
    return 0;
}

int test_fcntl_set_flags() {
    int pipe_fds[2];
    if (pipe(pipe_fds) < 0) {
        THROW_ERROR("failed to create a pipe");
    }

    fcntl(pipe_fds[0], F_SETFL, O_NONBLOCK);
    if ((fcntl(pipe_fds[0], F_GETFL, 0) != (O_NONBLOCK | O_RDONLY)) ||
            (fcntl(pipe_fds[1], F_GETFL, 0) !=  O_WRONLY)) {
        free_pipe(pipe_fds);
        THROW_ERROR("fcntl set flags failed");
    }

    free_pipe(pipe_fds);
    return 0;
}

int test_create_with_flags() {
    int pipe_fds[2];
    if (pipe2(pipe_fds, O_NONBLOCK) < 0) {
        THROW_ERROR("failed to create a pipe");
    }

    if ((fcntl(pipe_fds[0], F_GETFL, 0) != (O_NONBLOCK | O_RDONLY)) ||
            (fcntl(pipe_fds[1], F_GETFL, 0) != (O_NONBLOCK | O_WRONLY))) {
        free_pipe(pipe_fds);
        THROW_ERROR("create flags failed\n");
    }

    free_pipe(pipe_fds);
    return 0;
}

int test_select_timeout() {
    fd_set rfds;

    int pipe_fds[2];
    if (pipe(pipe_fds) < 0) {
        THROW_ERROR("failed to create a pipe");
    }

    struct timeval tv = { .tv_sec = 1, .tv_usec = 0 };

    FD_ZERO(&rfds);
    FD_SET(pipe_fds[0], &rfds);
    struct timeval tv_start, tv_end;
    gettimeofday(&tv_start, NULL);
    select(pipe_fds[0] + 1, &rfds, NULL, NULL, &tv);
    gettimeofday(&tv_end, NULL);
    double total_s = tv_end.tv_sec - tv_start.tv_sec;
    if (total_s < 1) {
        printf("time consumed is %f\n",
               total_s + (double)(tv_end.tv_usec - tv_start.tv_usec) / 1000000);
        THROW_ERROR("select timer does not work correctly");
    }

    free_pipe(pipe_fds);
    return 0;
}

int test_epoll_timeout() {
    int pipe_fds[2];
    if (pipe(pipe_fds) < 0) {
        THROW_ERROR("failed to create a pipe");
    }
    int pipe_read_fd = pipe_fds[0];
    int pipe_write_fd = pipe_fds[1];

    int ep_fd = epoll_create1(0);
    if (ep_fd < 0) {
        THROW_ERROR("failed to create an epoll");
    }

    int ret;
    struct epoll_event event;

    event.events = EPOLLIN; // we want the write end to be readable
    event.data.u32 = pipe_write_fd;
    ret = epoll_ctl(ep_fd, EPOLL_CTL_ADD, pipe_write_fd, &event);
    if (ret < 0) {
        THROW_ERROR("failed to do epoll ctl");
    }

    event.events = EPOLLOUT; // we want the read end to be writable
    event.data.u32 = pipe_read_fd;
    ret = epoll_ctl(ep_fd, EPOLL_CTL_ADD, pipe_read_fd, &event);
    if (ret < 0) {
        THROW_ERROR("failed to do epoll ctl");
    }

    // We are waiting for the write end to be readable or the read end to be
    // writable, which can never happen. So the epoll_wait must end with
    // timeout.
    errno = 0;
    struct epoll_event events[2];
    ret = epoll_wait(ep_fd, events, ARRAY_SIZE(events), 10 /* ms */);
    if (ret != 0 || errno != 0) {
        THROW_ERROR("failed to do epoll ctl");
    }

    free_pipe(pipe_fds);
    close(ep_fd);
    return 0;
}

int test_poll_timeout() {
    // Start the timer
    struct timeval tv_start, tv_end;
    gettimeofday(&tv_start, NULL);
    int fds[2];
    if (pipe(fds) < 0) {
        THROW_ERROR("pipe failed");
    }
    struct pollfd polls[] = {
        { .fd = fds[0], .events = POLLOUT },
        { .fd = fds[1], .events = POLLIN }
    };

    poll(polls, 2, 1000);
    // Stop the timer
    gettimeofday(&tv_end, NULL);
    double total_s = tv_end.tv_sec - tv_start.tv_sec;
    if ((int)total_s < 1) {
        printf("time consumed is %f\n",
               total_s + (double)(tv_end.tv_usec - tv_start.tv_usec) / 1000000);
        THROW_ERROR("poll timer does not work correctly");
    }
    return 0;
}

int test_select_no_timeout() {
    fd_set wfds;
    int ret = 0;

    int pipe_fds[2];
    if (pipe(pipe_fds) < 0) {
        THROW_ERROR("failed to create a pipe");
    }

    FD_ZERO(&wfds);
    FD_SET(pipe_fds[1], &wfds);
    ret = select(pipe_fds[1] + 1, NULL, &wfds, NULL, NULL);
    if (ret != 1) {
        free_pipe(pipe_fds);
        THROW_ERROR("select failed");
    }

    if (FD_ISSET(pipe_fds[1], &wfds) == 0) {
        free_pipe(pipe_fds);
        THROW_ERROR("bad select return");
    }

    free_pipe(pipe_fds);
    return 0;
}

int test_poll_no_timeout() {
    int pipe_fds[2];
    if (pipe(pipe_fds) < 0) {
        THROW_ERROR("failed to create a pipe");
    }
    struct pollfd polls[] = {
        { .fd = pipe_fds[0], .events = POLLIN },
        { .fd = pipe_fds[1], .events = POLLOUT },
        { .fd = pipe_fds[1], .events = POLLOUT },
    };
    int ret = poll(polls, 3, -1);
    if (ret < 0) { THROW_ERROR("poll error"); }

    if (polls[0].revents != 0 || (polls[1].revents & POLLOUT) == 0 ||
            (polls[2].revents & POLLOUT) == 0 || ret != 2) { THROW_ERROR("wrong return events"); }
    return 0;
}

int test_epoll_no_timeout() {
    int pipe_fds[2];
    if (pipe(pipe_fds) < 0) {
        THROW_ERROR("failed to create a pipe");
    }
    int pipe_read_fd = pipe_fds[0];
    int pipe_write_fd = pipe_fds[1];

    int ep_fd = epoll_create1(0);
    if (ep_fd < 0) {
        THROW_ERROR("failed to create an epoll");
    }

    int ret;
    struct epoll_event event;

    event.events = EPOLLOUT; // writable
    event.data.u32 = pipe_write_fd;
    ret = epoll_ctl(ep_fd, EPOLL_CTL_ADD, pipe_write_fd, &event);
    if (ret < 0) {
        THROW_ERROR("failed to do epoll ctl");
    }

    event.events = EPOLLIN; // readable
    event.data.u32 = pipe_read_fd;
    ret = epoll_ctl(ep_fd, EPOLL_CTL_ADD, pipe_read_fd, &event);
    if (ret < 0) {
        THROW_ERROR("failed to do epoll ctl");
    }

    struct epoll_event events[2];
    ret = epoll_wait(ep_fd, events, ARRAY_SIZE(events), -1);
    // pipe_write_fd is ready, while pipe_read_fd is not
    if (ret != 1) {
        THROW_ERROR("failed to do epoll ctl");
    }

    free_pipe(pipe_fds);
    close(ep_fd);
    return 0;
}

int test_select_read_write() {
    int pipe_fds[2];
    if (pipe(pipe_fds) < 0) {
        THROW_ERROR("failed to create a pipe");
    }

    int pipe_rd_fd = pipe_fds[0];
    int pipe_wr_fd = pipe_fds[1];

    posix_spawn_file_actions_t file_actions;
    posix_spawn_file_actions_init(&file_actions);
    posix_spawn_file_actions_adddup2(&file_actions, pipe_wr_fd, STDOUT_FILENO);
    posix_spawn_file_actions_addclose(&file_actions, pipe_rd_fd);

    const char *msg = "Echo!\n";
    const char *child_prog = "/bin/hello_world";
    const char *child_argv[3] = { child_prog, msg, NULL };
    int child_pid;
    if (posix_spawn(&child_pid, child_prog, &file_actions,
                    NULL, (char *const *)child_argv, NULL) < 0) {
        THROW_ERROR("failed to spawn a child process");
    }
    close(pipe_wr_fd);

    const char *expected_str = msg;
    size_t expected_len = strlen(expected_str);
    char actual_str[32] = {0};
    fd_set rfds;

    FD_ZERO(&rfds);
    FD_SET(pipe_fds[0], &rfds);
    if (select(pipe_fds[0] + 1, &rfds, NULL, NULL, NULL) <= 0) {
        free_pipe(pipe_fds);
        THROW_ERROR("select failed");
    }

    if (read(pipe_rd_fd, actual_str, sizeof(actual_str) - 1) < 0) {
        THROW_ERROR("reading pipe failed");
    };

    if (strncmp(expected_str, actual_str, expected_len) != 0) {
        THROW_ERROR("received string is not as expected");
    }

    close(pipe_rd_fd);

    int status = 0;
    if (wait4(child_pid, &status, 0, NULL) < 0) {
        THROW_ERROR("failed to wait4 the child process");
    }
    return 0;
}

int test_ioctl_fionread() {
    int pipe_fds[2];
    if (pipe(pipe_fds) < 0) {
        THROW_ERROR("failed to create a pipe");
    }

    int pipe_rd_fd = pipe_fds[0];
    int pipe_wr_fd = pipe_fds[1];

    posix_spawn_file_actions_t file_actions;
    posix_spawn_file_actions_init(&file_actions);
    posix_spawn_file_actions_adddup2(&file_actions, pipe_wr_fd, STDOUT_FILENO);
    posix_spawn_file_actions_addclose(&file_actions, pipe_rd_fd);

    const char *msg = "Echo!\n";
    const char *child_prog = "/bin/hello_world";
    const char *child_argv[3] = { child_prog, msg, NULL };
    int child_pid;
    if (posix_spawn(&child_pid, child_prog, &file_actions,
                    NULL, (char *const *)child_argv, NULL) < 0) {
        THROW_ERROR("failed to spawn a child process");
    }
    int status = 0;
    if (wait4(child_pid, &status, 0, NULL) < 0) {
        THROW_ERROR("failed to wait4 the child process");
    }

    close(pipe_wr_fd);

    const char *expected_str = msg;
    size_t expected_len = strlen(expected_str);
    char actual_str[32] = {0};

    int data_len_ready = 0;
    if ( ioctl(pipe_rd_fd, FIONREAD, &data_len_ready) < 0 ) {
        THROW_ERROR("ioctl FIONREAD failed");
    }

    // data_len_ready will include '\0'
    if (data_len_ready - 1 != expected_len) {
        THROW_ERROR("ioctl FIONREAD value not match");
    }

    if (read(pipe_rd_fd, actual_str, sizeof(actual_str) - 1) < 0) {
        THROW_ERROR("reading pipe failed");
    };

    if (strncmp(expected_str, actual_str, expected_len) != 0) {
        THROW_ERROR("received string is not as expected");
    }

    close(pipe_rd_fd);
    return 0;
}

// ============================================================================
// Test suite
// ============================================================================
static test_case_t test_cases[] = {
    TEST_CASE(test_fstat),
    TEST_CASE(test_fcntl_get_flags),
    TEST_CASE(test_fcntl_set_flags),
    TEST_CASE(test_create_with_flags),
    TEST_CASE(test_select_timeout),
    TEST_CASE(test_poll_timeout),
    TEST_CASE(test_epoll_timeout),
    TEST_CASE(test_select_no_timeout),
    TEST_CASE(test_poll_no_timeout),
    TEST_CASE(test_epoll_no_timeout),
    TEST_CASE(test_select_read_write),
    TEST_CASE(test_ioctl_fionread),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
