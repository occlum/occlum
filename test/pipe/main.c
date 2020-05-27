#include <sys/syscall.h>
#include <sys/wait.h>
#include <fcntl.h>
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

int test_read_write() {
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
    ssize_t actual_len;
    do {
        actual_len = read(pipe_rd_fd, actual_str, sizeof(actual_str) - 1);
    } while (actual_len == 0);
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

// ============================================================================
// Test suite
// ============================================================================
static test_case_t test_cases[] = {
    TEST_CASE(test_fcntl_get_flags),
    TEST_CASE(test_fcntl_set_flags),
    TEST_CASE(test_create_with_flags),
    TEST_CASE(test_read_write),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
