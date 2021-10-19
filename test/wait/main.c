#define _GNU_SOURCE
#include <sys/wait.h>
#include <errno.h>
#include <spawn.h>
#include <stdbool.h>
#include <stdlib.h>
#include "test.h"

static int test_wait_no_children() {
    int status = 0;
    int ret = wait(&status);
    if (ret != -1 || errno != ECHILD) {
        THROW_ERROR("wait no children error");
    }
    return 0;
}

static int test_wait_nohang() {
    int status = 0;
    int ret = waitpid(-1, &status, WNOHANG);
    if (ret != -1 || errno != ECHILD) {
        THROW_ERROR("wait no children with NOHANG error");
    }

    int child_pid = 0;
    // /bin/sleep lasts more than 1 sec
    if (posix_spawn(&child_pid, "/bin/sleep", NULL, NULL, NULL, NULL) < 0) {
        THROW_ERROR("posix_spawn child error");
    }

    ret = waitpid(child_pid, &status, WNOHANG);
    if (ret != 0) {
        THROW_ERROR("wait child with NOHANG error");
    }

    sleep(2);
    // The child process should exit
    ret = waitpid(child_pid, &status, WNOHANG);
    if (ret != child_pid) {
        THROW_ERROR("wait child with NOHANG error");
    }
    return 0;
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_wait_no_children),
    TEST_CASE(test_wait_nohang),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
