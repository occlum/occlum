#include <fcntl.h>
#include <libgen.h>
#include <unistd.h>
#include <stdio.h>
#include "test.h"

// ============================================================================
// Helper function
// ============================================================================

static int remove_file(const char *file_path) {
    int ret;

    ret = unlink(file_path);
    if (ret < 0) {
        THROW_ERROR("failed to unlink the created file");
    }
    return 0;
}

// ============================================================================
// Test cases for open
// ============================================================================

static int __test_open(const char *file_path, int flags, int mode) {
    int fd = open(file_path, flags, mode);
    if (fd < 0) {
        THROW_ERROR("failed to open a file");
    }
    close(fd);
    return 0;
}

static int __test_openat_with_abs_path(const char *file_path, int flags, int mode) {
    int fd = openat(AT_FDCWD, file_path, flags, mode);
    if (fd < 0) {
        THROW_ERROR("failed to openat a file with abs path");
    }
    close(fd);
    return 0;
}

static int __test_openat_with_dirfd(const char *file_path, int flags, int mode) {
    char dir_buf[128] = { 0 };
    char base_buf[128] = { 0 };
    char *dir_name, *file_name;
    int dirfd, fd, ret;

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
    fd = openat(dirfd, file_name, flags, mode);
    if (fd < 0) {
        close(dirfd);
        THROW_ERROR("failed to openat a file with dirfd");
    }
    close(dirfd);
    close(fd);
    return 0;
}

typedef int(*test_open_func_t)(const char *, int, int);

static int test_open_framework(test_open_func_t fn) {
    const char *file_path = "/root/test_filesystem_open.txt";
    int flags = O_RDONLY | O_CREAT | O_TRUNC;
    int mode = 00666;

    if (fn(file_path, flags, mode) < 0) {
        return -1;
    }
    if (remove_file(file_path) < 0) {
        return -1;
    }
    return 0;
}

static int test_open() {
    return test_open_framework(__test_open);
}

static int test_openat_with_abs_path() {
    return test_open_framework(__test_openat_with_abs_path);
}

static int test_openat_with_dirfd() {
    return test_open_framework(__test_openat_with_dirfd);
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_open),
    TEST_CASE(test_openat_with_abs_path),
    TEST_CASE(test_openat_with_dirfd),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
