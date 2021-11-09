#include <sys/types.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <stdbool.h>
#include <errno.h>
#include <fcntl.h>
#include "test_fs.h"

// ============================================================================
// The test case of readdir
// ============================================================================

static int test_readdir() {
    char expected_entries[9][NAME_MAX] = {
        "bin",
        "dev",
        "host",
        "lib",
        "lib64",
        "proc",
        "opt",
        "root",
        "tmp",
    };

    if (check_readdir_with_expected_entries("/", expected_entries, 9) < 0) {
        THROW_ERROR("failed to check the result of readdir");
    }
    return 0;
}

static int getdents_with_big_enough_buffer(bool use_explicit_syscall) {
    int fd, len;
    char buf[64];

    fd = open("/", O_RDONLY | O_DIRECTORY);
    if (fd < 0) {
        THROW_ERROR("failed to open directory");
    }
    while (1) {
        if (use_explicit_syscall) {
            len = syscall(__NR_getdents, fd, buf, sizeof(buf));
#ifndef __GLIBC__
        } else {
            len = getdents(fd, (struct dirent *)buf, sizeof(buf));
#endif
        }
        if (len < 0) {
            close(fd);
            THROW_ERROR("failed to call getdents");
        } else if (len == 0) {
            // On end of directory, 0 is returned
            break;
        }
    }
    close(fd);
    return 0;
}

#ifndef __GLIBC__
static int test_getdents_with_big_enough_buffer() {
    bool use_explicit_syscall = false;
    return getdents_with_big_enough_buffer(use_explicit_syscall);
}
#endif

static int test_getdents_via_explicit_syscall_with_big_enough_buffer() {
    bool use_explicit_syscall = true;
    return getdents_with_big_enough_buffer(use_explicit_syscall);
}

static int getdents_with_too_small_buffer(bool use_explicit_syscall) {
    int fd, len;
    char buf[4];

    fd = open("/", O_RDONLY | O_DIRECTORY);
    if (fd < 0) {
        THROW_ERROR("failed to open directory");
    }
    if (use_explicit_syscall) {
        len = syscall(__NR_getdents, fd, buf, sizeof(buf));
#ifndef __GLIBC__
    } else {
        len = getdents(fd, (struct dirent *)buf, sizeof(buf));
#endif
    }
    if (len >= 0 || errno != EINVAL) {
        close(fd);
        THROW_ERROR("failed to call getdents with small buffer");
    }
    close(fd);
    return 0;
}

#ifndef __GLIBC__
static int test_getdents_with_too_small_buffer() {
    bool use_explicit_syscall = false;
    return getdents_with_too_small_buffer(use_explicit_syscall);
}
#endif

static int test_getdents_via_explicit_syscall_with_too_small_buffer() {
    bool use_explicit_syscall = true;
    return getdents_with_too_small_buffer(use_explicit_syscall);
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_readdir),
#ifndef __GLIBC__
    TEST_CASE(test_getdents_with_big_enough_buffer),
#endif
    TEST_CASE(test_getdents_via_explicit_syscall_with_big_enough_buffer),
#ifndef __GLIBC__
    TEST_CASE(test_getdents_with_too_small_buffer),
#endif
    TEST_CASE(test_getdents_via_explicit_syscall_with_too_small_buffer),
};

int main() {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
