#include <sys/stat.h>
#include <errno.h>
#include <fcntl.h>
#include <unistd.h>
#include <string.h>
#include <stdio.h>
#include "test.h"

// ============================================================================
// Helper function
// ============================================================================

static int create_file(const char *file_path) {
    int fd;
    int flags = O_RDONLY | O_CREAT | O_TRUNC;
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
// Test cases for hostfs
// ============================================================================

static int __test_write_read(const char *file_path) {
    char *write_str = "Write to hostfs successfully!";
    char read_buf[128] = { 0 };
    int fd;

    fd = open(file_path, O_WRONLY);
    if (fd < 0) {
        THROW_ERROR("failed to open a file to write");
    }
    if (write(fd, write_str, strlen(write_str)) <= 0) {
        THROW_ERROR("failed to write to the file");
    }
    close(fd);
    fd = open(file_path, O_RDONLY);
    if (fd < 0) {
        THROW_ERROR("failed to open a file to read");
    }
    if (read(fd, read_buf, sizeof(read_buf)) != strlen(write_str)) {
        THROW_ERROR("failed to read to the file");
    }
    if (strcmp(write_str, read_buf) != 0) {
        THROW_ERROR("the message read from the file is not as it was written");
    }
    close(fd);
    return 0;
}

static int __test_rename(const char *file_path) {
    char *rename_path = "/host/hostfs_rename.txt";
    struct stat stat_buf;
    int ret;

    ret = rename(file_path, rename_path);
    if (ret < 0) {
        THROW_ERROR("failed to rename");
    }
    ret = stat(file_path, &stat_buf);
    if (!(ret < 0 && errno == ENOENT)) {
        THROW_ERROR("stat should return ENOENT");
    }
    ret = stat(rename_path, &stat_buf);
    if (ret < 0) {
        THROW_ERROR("failed to stat the file");
    }
    if (rename(rename_path, file_path) < 0) {
        THROW_ERROR("failed to rename back");
    }
    return 0;
}

typedef int(*test_hostfs_func_t)(const char *);

static int test_hostfs_framework(test_hostfs_func_t fn) {
    const char *file_path = "/host/hostfs_test.txt";

    if (create_file(file_path) < 0) {
        return -1;
    }
    if (fn(file_path) < 0) {
        return -1;
    }
    if (remove_file(file_path) < 0) {
        return -1;
    }
    return 0;
}

static int test_write_read() {
    return test_hostfs_framework(__test_write_read);
}

static int test_rename() {
    return test_hostfs_framework(__test_rename);
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_write_read),
    TEST_CASE(test_rename),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
