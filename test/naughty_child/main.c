// This test file may used by other test to test children behaviour spawned.
#include <sys/types.h>
#include <signal.h>
#include <assert.h>
#include <stdlib.h>
#include <features.h>
#include <string.h>
#include <sys/stat.h>
#include "test.h"

char **g_argv;

void sigio_handler(int sig) {
    printf("[child] SIGIO is caught in child!\n");
}

void sigabort_handler(int sig) {
    printf("[child] sigabort is caught in child! This shouldn't happen!\n");
    exit(-1);
}

// Parent process has set the sigmask of this child process to block SIGABORT by inheritage or posix_spawnattr_t
int test_spawn_attribute_sigmask() {
    printf("[child] Run a child process with pid = %d and ppid = %d\n", getpid(), getppid());

#ifndef __GLIBC__
    // musl can perform extra checks
    struct __sigset_t current_block_sigmask;
    struct __sigset_t test;
#else
    sigset_t current_block_sigmask, test;
#endif

    sigprocmask(0, NULL, &current_block_sigmask);
    sigemptyset(&test);
    sigaddset(&test, SIGABRT);

#ifndef __GLIBC__
    if (current_block_sigmask.__bits[0] != test.__bits[0]) {
        THROW_ERROR("[child] signask in child process is wrong");
    }
#endif
    signal(SIGIO, sigio_handler);
    signal(SIGABRT, sigabort_handler);
    raise(SIGIO);
    raise(SIGABRT);

    printf("[child] child test_spawn_attribute_sigmask - [Ok]\n");
    return 0;
}

// Parent process will set the sigaction of SIGALRM and SIGILL to SIG_IGN and SIGIO to user-defined handler. Then use posix_spawn attribute to set
// SIGALRM to SIG_DEF.
// Child process should inherit the ignore action of SIGILL and change SIGALRM and SIGIO sigaction to SIG_DEF.
int test_spawn_attribute_sigdef() {
    struct sigaction action;

    sigaction(SIGALRM, NULL, &action);
    if (action.sa_handler != SIG_DFL) {
        THROW_ERROR("[child] sig handler of SIGALRM is wrong");
    }

    sigaction(SIGIO, NULL, &action);
    if (action.sa_handler != SIG_DFL) {
        THROW_ERROR("[child] sig handler of SIGIO is wrong");
    }

    sigaction(SIGILL, NULL, &action);
    if (action.sa_handler != SIG_IGN) {
        THROW_ERROR("[child] sig handler of SIGILL is wrong");
    }

    printf("[child] child test_spawn_attribute_sigdef - [Ok]\n");
    return 0;
}

int test_ioctl_fioclex() {
    int regular_file_fd = atoi(g_argv[3]);
    int pipe_reader_fd = atoi(g_argv[4]);
    int pipe_writer_fd = atoi(g_argv[5]);

    // regular file is set with ioctl FIONCLEX
    struct stat stat_buf;
    int ret = fstat(regular_file_fd, &stat_buf);
    if (ret != 0 || !S_ISREG(stat_buf.st_mode)) {
        THROW_ERROR("fstat regular file fd error");
    }

    // pipe reader is set with ioctl FIOCLEX
    ret = fstat(pipe_reader_fd, &stat_buf);
    if (ret != -1 || errno != EBADF) {
        THROW_ERROR("fstat pipe reader fd error");
    }

#if 0 // Need fstat support for pipe
    // pipe writer is set with default and should inherit by child
    ret = fstat(pipe_writer_fd, &stat_buf);
    if (ret != 0 || !S_ISFIFO(stat_buf.st_mode)) {
        THROW_ERROR("fstat pipe writer fd error");
    }
#endif

    return 0;
}

// ============================================================================
// Test suite
// ============================================================================

#define TEST_NAME_MAX 20

int start_test(const char *test_name) {
    if (strcmp(test_name, "sigmask") == 0) {
        return test_spawn_attribute_sigmask();
    } else if (strcmp(test_name, "sigdef") == 0) {
        return test_spawn_attribute_sigdef();
    } else if (strcmp(test_name, "fioclex") == 0) {
        return test_ioctl_fioclex();
    } else {
        fprintf(stderr, "[child] test case not found\n");
        return -1;
    }
}

void print_usage() {
    fprintf(stderr, "Usage:\n nauty_child [-t testcase1] [-t testcase2] ...\n\n");
    fprintf(stderr, " Now support testcase: <sigmask, sigdef, fioclex>\n");
}

int main(int argc, char *argv[]) {
    if (argc <= 1) {
        print_usage();
        return 0;
    }

    g_argv = argv;
    int opt;
    char *testcase_name = calloc(1, TEST_NAME_MAX);
    while ((opt = getopt(argc, argv, "t:")) != -1) {
        switch (opt) {
            case 't': {
                int len = strlen(optarg);
                if (len >= TEST_NAME_MAX) {
                    THROW_ERROR("[child] test case name too long");
                }
                memset(testcase_name, 0, TEST_NAME_MAX);
                strncpy(testcase_name, optarg, len + 1);
                printf("[child] start testcase: %s\n", testcase_name);
                int ret = start_test(testcase_name);
                if (ret != 0) {
                    THROW_ERROR("[child] test case failure");
                }
            }
            break;
            default:
                print_usage();
                exit(-1);
        }
    }

    free(testcase_name);
    return 0;
}
