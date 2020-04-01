#include <sys/stat.h>
#include <errno.h>
#include <fcntl.h>
#include <unistd.h>
#include <stdio.h>
#include "test.h"

// ============================================================================
// Helper function
// ============================================================================

static int create_file(const char *file_path) {
    int fd;
    int flags = O_RDONLY | O_CREAT| O_TRUNC;
    int mode = 00444;

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

static int __test_chmod(const char *file_path) {
    struct stat stat_buf;
    mode_t mode = 00664;
    int ret;

    ret = chmod(file_path, mode);
    if (ret < 0) {
        THROW_ERROR("failed to chmod file");
    }
    ret = stat(file_path, &stat_buf);
    if (ret < 0) {
        THROW_ERROR("failed to stat file");
    }
    if ((stat_buf.st_mode & 07777) != mode) {
        THROW_ERROR("check chmod result failed");
    }
    return 0;
}

static int __test_fchmod(const char *file_path) {
    struct stat stat_buf;
    mode_t mode = 00664;
    int fd, ret;

    fd = open(file_path, O_RDONLY);
    if (fd < 0) {
        THROW_ERROR("failed to open file");
    }
    ret = fchmod(fd, mode);
    if (ret < 0) {
        close(fd);
        THROW_ERROR("failed to fchmod file");
    }
    close(fd);
    ret = stat(file_path, &stat_buf);
    if (ret < 0) {
        THROW_ERROR("failed to stat file");
    }
    if ((stat_buf.st_mode & 07777) != mode) {
        THROW_ERROR("check fchmod result failed");
    }
    return 0;
}

typedef int(*test_chmod_func_t)(const char *);

static int test_chmod_framework(test_chmod_func_t fn) {
    const char *file_path = "/root/test_filesystem_chmod.txt";

    if (create_file(file_path) < 0)
        return -1;
    if (fn(file_path) < 0)
        return -1;
    if (remove_file(file_path) < 0)
        return -1;
    return 0;
}

static int test_chmod() {
    return test_chmod_framework(__test_chmod);
}

static int test_fchmod() {
    return test_chmod_framework(__test_fchmod);
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_chmod),
    TEST_CASE(test_fchmod),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
