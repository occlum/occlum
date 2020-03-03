#include <sys/stat.h>
#include <errno.h>
#include <fcntl.h>
#include <libgen.h>
#include <unistd.h>
#include <stdio.h>
#include "test.h"

// ============================================================================
// Helper function
// ============================================================================

static int create_file(const char *file_path) {
    int fd;
    int flags = O_RDONLY | O_CREAT| O_TRUNC;
    int mode = 00666;

    fd = open(file_path, flags, mode);
    if (fd < 0) {
        THROW_ERROR("failed to create a file");
    }
    close(fd);
    return 0;
}

static int remove_file(const char *file_path) {
    int ret;

    ret = unlink(file_path);
    if (ret < 0) {
        THROW_ERROR("failed to unlink the created file");
    }
    return 0;
}

// ============================================================================
// Test cases for stat
// ============================================================================

static int __test_stat(const char *file_path) {
    struct stat stat_buf;
    int ret;

    ret = stat(file_path, &stat_buf);
    if (ret < 0) {
        THROW_ERROR("failed to stat file");
    }
    return 0;
}

static int __test_fstat(const char *file_path) {
    struct stat stat_buf;
    int fd, ret;
    int flags = O_RDONLY;

    fd = open(file_path, flags);
    if (fd < 0) {
        THROW_ERROR("failed to open file");
    }
    ret = fstat(fd, &stat_buf);
    if (ret < 0) {
        close(fd);
        THROW_ERROR("failed to fstat file");
    }
    close(fd);
    return 0;
}

static int __test_lstat(const char *file_path) {
    struct stat stat_buf;
    int ret;

    ret = lstat(file_path, &stat_buf);
    if (ret < 0) {
        THROW_ERROR("failed to lstat file");
    }
    return 0;
}

static int __test_fstatat_with_abs_path(const char *file_path) {
    struct stat stat_buf;
    int ret;

    ret = fstatat(AT_FDCWD, file_path, &stat_buf, 0);
    if (ret < 0) {
        THROW_ERROR("failed to fstatat file with abs path");
    }
    return 0;
}

static int __test_fstatat_with_empty_path(const char *file_path) {
    struct stat stat_buf;
    int fd, ret;

    ret = fstatat(AT_FDCWD, "", &stat_buf, 0);
    if (!(ret < 0 && errno == ENOENT)) {
        THROW_ERROR("fstatat with empty path should return ENOENT");
    }

    fd = open(file_path, O_RDONLY);
    if (fd < 0) {
        THROW_ERROR("failed to open file");
    }
    ret = fstatat(fd, "", &stat_buf, AT_EMPTY_PATH);
    if (ret < 0) {
        close(fd);
        THROW_ERROR("failed to fstatat empty path with AT_EMPTY_PATH flags");
    }
    close(fd);
    return 0;
}

static int __test_fstatat_with_dirfd(const char *file_path) {
    struct stat stat_buf;
    char dir_buf[128] = { 0 };
    char base_buf[128] = { 0 };
    char *dir_name, *file_name;
    int dirfd, ret;

    ret = snprintf(dir_buf, sizeof(dir_buf), "%s", file_path);
    if (ret >= sizeof(dir_buf) || ret < 0) {
        THROW_ERROR("failed to copy file path to the dir buffer");
    }
    ret = snprintf(base_buf, sizeof(base_buf), "%s", file_path);
    if (ret >= sizeof(base_buf) || ret < 0) {
        THROW_ERROR("failed to copy file path to the base buffer");
    }
    dir_name = dirname(dir_buf);
    file_name = basename(base_buf);
    dirfd = open(dir_name, O_RDONLY);
    if (dirfd < 0) {
        THROW_ERROR("failed to open dir");
    }
    ret = fstatat(dirfd, file_name, &stat_buf, 0);
    if (ret < 0) {
        close(dirfd);
        THROW_ERROR("failed to fstatat file with dirfd");
    }
    close(dirfd);
    return 0;
}

typedef int(*test_stat_func_t)(const char *);

static int test_stat_framework(test_stat_func_t fn) {
    const char *file_path = "/root/test_filesystem_stat.txt";

    if (create_file(file_path) < 0)
        return -1;
    if (fn(file_path) < 0)
        return -1;
    if (remove_file(file_path) < 0)
        return -1;
    return 0;
}

static int test_stat() {
    return test_stat_framework(__test_stat);
}

static int test_fstat() {
    return test_stat_framework(__test_fstat);
}

static int test_lstat() {
    return test_stat_framework(__test_lstat);
}

static int test_fstatat_with_abs_path() {
    return test_stat_framework(__test_fstatat_with_abs_path);
}

static int test_fstatat_with_empty_path() {
    return test_stat_framework(__test_fstatat_with_empty_path);
}

static int test_fstatat_with_dirfd() {
    return test_stat_framework(__test_fstatat_with_dirfd);
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_stat),
    TEST_CASE(test_fstat),
    TEST_CASE(test_lstat),
    TEST_CASE(test_fstatat_with_abs_path),
    TEST_CASE(test_fstatat_with_empty_path),
    TEST_CASE(test_fstatat_with_dirfd),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
