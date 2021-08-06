#include <sys/stat.h>
#include <errno.h>
#include <fcntl.h>
#include "test_fs.h"

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

static int check_create_file_with_umask(const char *file_path, mode_t mask) {
    mode_t mode = 00666;
    int fd = creat(file_path, mode);
    if (fd < 0) {
        THROW_ERROR("failed to create file");
    }

    struct stat stat_buf;
    if (fstat(fd, &stat_buf) < 0) {
        THROW_ERROR("failed to stat file");
    }
    mode_t actual_mode = stat_buf.st_mode & 00777;
    if (actual_mode != (mode & ~mask)) {
        THROW_ERROR("failed to check the mode with umask(%o), actual_mode is: %o", mask,
                    actual_mode);
    }

    return 0;
}

// ============================================================================
// Test cases for umask
// ============================================================================

#define DEFAULT_UMASK (00022)

static int __test_create_file_with_default_umask(const char *file_path) {
    if (check_create_file_with_umask(file_path, DEFAULT_UMASK) < 0) {
        THROW_ERROR("failed to check default umask");
    }

    return 0;
}

static int __test_umask(const char *file_path) {
    mode_t new_mask = 00066;
    int old_mask = umask(new_mask);
    if (old_mask != DEFAULT_UMASK) {
        THROW_ERROR("failed to get correct default mask");
    }

    if (check_create_file_with_umask(file_path, new_mask) < 0) {
        THROW_ERROR("failed to check default umask");
    }

    return 0;
}

typedef int(*test_file_func_t)(const char *);

static int test_file_framework(test_file_func_t fn) {
    const char *file_path = "/root/test_filesystem_umask.txt";

    if (fn(file_path) < 0) {
        return -1;
    }
    if (remove_file(file_path) < 0) {
        return -1;
    }
    return 0;
}

static int test_create_file_with_default_umask() {
    return test_file_framework(__test_create_file_with_default_umask);
}

static int test_umask() {
    return test_file_framework(__test_umask);
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_create_file_with_default_umask),
    TEST_CASE(test_umask),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
