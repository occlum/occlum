#include <sys/types.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <dirent.h>
#include <stdbool.h>
#include <errno.h>
#include <fcntl.h>
#include "test_fs.h"

// ============================================================================
// The test case of readdir
// ============================================================================

static int test_readdir() {
    struct dirent *dp;
    DIR *dirp;

    dirp = opendir("/");
    if (dirp == NULL) {
        THROW_ERROR("failed to open directory");
    }
    while (1) {
        errno = 0;
        dp = readdir(dirp);
        if (dp == NULL) {
            if (errno != 0) {
                closedir(dirp);
                THROW_ERROR("faild to call readdir");
            }
            break;
        }
    }
    closedir(dirp);
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
        } else {
            len = getdents(fd, (struct dirent *)buf, sizeof(buf));
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

static int test_getdents_with_big_enough_buffer() {
    bool use_explicit_syscall = false;
    return getdents_with_big_enough_buffer(use_explicit_syscall);
}

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
    } else {
        len = getdents(fd, (struct dirent *)buf, sizeof(buf));
    }
    if (len >= 0 || errno != EINVAL) {
        close(fd);
        THROW_ERROR("failed to call getdents with small buffer");
    }
    close(fd);
    return 0;
}

static int test_getdents_with_too_small_buffer() {
    bool use_explicit_syscall = false;
    return getdents_with_too_small_buffer(use_explicit_syscall);
}

static int test_getdents_via_explicit_syscall_with_too_small_buffer() {
    bool use_explicit_syscall = true;
    return getdents_with_too_small_buffer(use_explicit_syscall);
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_readdir),
    TEST_CASE(test_getdents_with_big_enough_buffer),
    TEST_CASE(test_getdents_via_explicit_syscall_with_big_enough_buffer),
    TEST_CASE(test_getdents_with_too_small_buffer),
    TEST_CASE(test_getdents_via_explicit_syscall_with_too_small_buffer),
};

int main() {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
