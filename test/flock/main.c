#include <string.h>
#include <stdio.h>
#include <errno.h>
#include <spawn.h>
#include <stdlib.h>
#include <sys/wait.h>
#include <sys/file.h>
#include <fcntl.h>
#include <unistd.h>
#include "test.h"

// ============================================================================
// Helper structs & variables & functions
// ============================================================================

const char *g_file_path = "/root/test_flock_file.txt";
int g_fd;

static int open_or_create_file() {
    int flags = O_RDWR | O_CREAT;
    int mode = 00666;

    int fd = open(g_file_path, flags, mode);
    if (fd < 0) {
        THROW_ERROR("failed to open or create file");
    }
    return fd;
}

static int remove_file() {
    if (unlink(g_file_path) < 0) {
        THROW_ERROR("failed to unlink the created file");
    }
    return 0;
}

// ============================================================================
// Test cases for FLOCK
// ============================================================================

static int test_invalid_operation() {
    // Check the operation with expected errno
    int ops_with_expected_errno[5][2] = {
        {LOCK_SH | LOCK_EX, EINVAL},
        {LOCK_SH | LOCK_UN, EINVAL},
        {LOCK_EX | LOCK_UN, EINVAL},
        {LOCK_SH | 0x1000, EINVAL},
        {LOCK_NB, EINVAL},
    };
    int row_cnt = (sizeof(ops_with_expected_errno) / sizeof(int)) /
                  (sizeof(ops_with_expected_errno[0]) / sizeof(int));
    for (int i = 0; i < row_cnt; i++) {
        int ops = ops_with_expected_errno[i][0];
        int expected_errno = ops_with_expected_errno[i][1];
        errno = 0;

        int ret = flock(g_fd, ops);
        if (!(ret < 0 && errno == expected_errno)) {
            THROW_ERROR("failed to check flock with invalid operation");
        }
    }
    return 0;
}

static int test_lock() {
    int operation = LOCK_EX | LOCK_NB;
    if (flock(g_fd, operation) < 0) {
        THROW_ERROR("failed to lock file");
    }

    operation = LOCK_SH | LOCK_NB;
    if (flock(g_fd, operation) < 0) {
        THROW_ERROR("failed to lock file");
    }
    return 0;
}

static int test_spawn_child_and_unlock() {
    int status, child_pid;

    char g_fd_buf[16];
    sprintf(g_fd_buf, "%d", g_fd);
    const char *child_argv[3] = {
        "flock",
        g_fd_buf,
        NULL
    };
    int ret = posix_spawn(&child_pid,
                          "/bin/flock", NULL, NULL,
                          (char *const *)child_argv,
                          NULL);
    if (ret < 0) {
        THROW_ERROR("spawn process error");
    }
    printf("Spawn a child process with pid=%d\n", child_pid);

    // Sleep 3s for the child to run flock test and wait, is 3s enough?
    sleep(3);

    // Unlock the flock will cause child process to finish running
    int operation = LOCK_UN;
    if (flock(g_fd, operation) < 0) {
        THROW_ERROR("failed to unlock the lock");
    }

    // Wait for child exit
    ret = wait4(child_pid, &status, 0, NULL);
    if (ret < 0) {
        THROW_ERROR("failed to wait4 the child process");
    }
    if (!(WIFEXITED(status) && WEXITSTATUS(status) == 0)) {
        THROW_ERROR("test cases in child faild");
    }

    // The lock will be unlocked on child exit, so we can lock again
    operation = LOCK_EX;
    ret = flock(g_fd, operation);
    if (ret < 0 && errno != EINTR) {
        THROW_ERROR("failed to check the result of flock");
    }

    return 0;
}

// ============================================================================
// Child Test cases
// ============================================================================

static int test_child_lock_wait() {
    // Child open the file with new fd
    int new_fd = open_or_create_file();

    int operation = LOCK_SH | LOCK_NB;
    if (flock(new_fd, operation) < 0) {
        THROW_ERROR("failed set shared flock");
    }

    operation = LOCK_UN;
    if (flock(new_fd, operation) < 0) {
        THROW_ERROR("failed to unlock the new lock");
    }

    // Child inherits file table, so it can change the old lock to exclusive
    operation = LOCK_EX | LOCK_NB;
    if (flock(g_fd, operation) < 0) {
        THROW_ERROR("failed change the lock type to exclusive lock");
    }

    // Try to set new lock
    operation = LOCK_SH | LOCK_NB;
    int res = flock(new_fd, operation);
    if (!(res < 0 && errno == EAGAIN)) {
        THROW_ERROR("failed to check the file lock state");
    }
    // Child will wait here
    operation = LOCK_SH;
    res = flock(new_fd, operation);
    if (res < 0 && errno != EINTR) {
        THROW_ERROR("failed to check the result of flock with conflict lock");
    }
    return 0;
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_invalid_operation),
    TEST_CASE(test_lock),
    TEST_CASE(test_spawn_child_and_unlock),
};

static test_case_t child_test_cases[] = {
    TEST_CASE(test_child_lock_wait),
};

int main(int argc, const char *argv[]) {
    // Test argc
    if (argc == 2) {
        g_fd = atoi(argv[1]);
        if (test_suite_run(child_test_cases, ARRAY_SIZE(child_test_cases)) < 0) {
            THROW_ERROR("failed run child test");
        }
    } else {
        g_fd = open_or_create_file();
        if (g_fd < 0) {
            THROW_ERROR("failed to open/create file");
        }
        if (test_suite_run(test_cases, ARRAY_SIZE(test_cases)) < 0) {
            THROW_ERROR("failed run test");
        }
        close(g_fd);
        if (remove_file() < 0) {
            THROW_ERROR("failed to remove file after test");
        }
    }
    return 0;
}
