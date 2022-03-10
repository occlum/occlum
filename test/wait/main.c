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
    // /bin/wait_child lasts for 2 sec
    if (posix_spawn(&child_pid, "/bin/wait_child", NULL, NULL, NULL, NULL) < 0) {
        THROW_ERROR("posix_spawn child error");
    }

    ret = waitpid(child_pid, &status, WNOHANG);
    if (ret != 0) {
        THROW_ERROR("wait child with NOHANG error");
    }

    sleep(3);
    // The child process should exit
    ret = waitpid(child_pid, &status, WNOHANG);
    if (ret != child_pid) {
        THROW_ERROR("wait child with NOHANG error");
    }
    return 0;
}

// NOTE: WUNTRACED is same as WSTOPPED
// TODO: Support WUNTRACED and WCONTINUED and enable this test case
static int test_wait_untraced_and_continued() {
    int status = 0;
    int ret = waitpid(-1, &status, WNOHANG);
    if (ret != -1 || errno != ECHILD) {
        THROW_ERROR("wait no children with NOHANG error");
    }

    int child_pid = 0;
    if (posix_spawn(&child_pid, "/bin/sleep", NULL, NULL, NULL, NULL) < 0) {
        THROW_ERROR("posix_spawn child error");
    }

    ret = waitpid(child_pid, &status, WNOHANG);
    if (ret != 0) {
        THROW_ERROR("wait child with NOHANG error");
    }

    kill(child_pid, SIGSTOP);
    // WUNTRACED will get child_pid status
    ret = waitpid(child_pid, &status, WUNTRACED);
    printf("ret = %d, status = %d\n", ret, status);
    if (ret != child_pid || !WIFSTOPPED(status) || WSTOPSIG(status) != SIGSTOP ) {
        THROW_ERROR("wait child status error");
    }

    // Let child get back to running by sending SIGCONT
    kill(child_pid, SIGCONT);
    ret = waitpid(child_pid, &status, WCONTINUED);
    printf("ret = %d, status = %d\n", ret, status);
    if (ret != child_pid || !WIFCONTINUED(status)) {
        THROW_ERROR("wait child status error");
    }

    sleep(2);
    // The child process should exit
    ret = waitpid(child_pid, &status, WNOHANG | WUNTRACED);
    printf("ret = %d, status = %d\n", ret, status);
    if (ret != child_pid || !WIFEXITED(status) ) {
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
    // TODO: Enable this test case
    // TEST_CASE(test_wait_untraced_and_continued),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
