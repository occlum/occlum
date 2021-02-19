#include <sys/types.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <dirent.h>
#include <stdbool.h>
#include <errno.h>
#include <fcntl.h>
#include "test_fs.h"

// ============================================================================
// Helper function
// ============================================================================

#define NUM 9
static bool check_dir_entries(char entries[][256], int entry_cnt) {
    char *expected_entries[NUM] = {
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
    for (int i = 0; i < NUM; i++) {
        bool find_entry = false;
        for (int j = 0; j < entry_cnt; j++) {
            if (strncmp(expected_entries[i], entries[j], strlen(expected_entries[i])) == 0) {
                find_entry = true;
                break;
            }
        }
        if (!find_entry) {
            return false;
        }
    }
    return true;
}

// ============================================================================
// The test case of readdir
// ============================================================================

static int test_readdir() {
    struct dirent *dp;
    DIR *dirp;
    char entries[32][256] = { 0 };

    dirp = opendir("/");
    if (dirp == NULL) {
        THROW_ERROR("failed to open directory");
    }
    int entry_cnt = 0;
    while (1) {
        errno = 0;
        dp = readdir(dirp);
        if (dp == NULL) {
            if (errno != 0) {
                closedir(dirp);
                THROW_ERROR("failed to call readdir");
            }
            break;
        }
        strncpy(entries[entry_cnt], dp->d_name, 256);
        ++entry_cnt;
    }
    if (!check_dir_entries(entries, entry_cnt)) {
        THROW_ERROR("failed to check the result of readdir");
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
