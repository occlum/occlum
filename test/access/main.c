#include <sys/stat.h>
#include <errno.h>
#include <fcntl.h>
#include "test_fs.h"

// ============================================================================
// Helper function
// ============================================================================

static int create_file(const char *file_path, mode_t mode) {
    int flags = O_RDONLY | O_CREAT | O_TRUNC;
    int fd;

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
// Test cases for access
// ============================================================================

static int __test_access(const char *file_path) {
    if (access(file_path, F_OK) < 0) {
        THROW_ERROR("failed to access file with F_OK");
    }
    if (access(file_path, R_OK | W_OK) < 0) {
        THROW_ERROR("failed to access file");
    }
    if (access(file_path, R_OK | W_OK | X_OK) >= 0 || errno != EACCES) {
        THROW_ERROR("failed to access file with X_OK");
    }
    if (access(file_path, 0xF) >= 0 || errno != EINVAL) {
        THROW_ERROR("failed to access file with invalid mode");
    }
    if (remove_file(file_path) < 0) {
        return -1;
    }
    if (access(file_path, F_OK) >= 0 || errno != ENOENT) {
        THROW_ERROR("failed to access file after unlink");
    }
    return 0;
}

static int __test_faccessat_with_abs_path(const char *file_path) {
    if (faccessat(AT_FDCWD, file_path, F_OK, 0) < 0) {
        THROW_ERROR("failed to faccessat file with abs path");
    }
    if (faccessat(-1, file_path, F_OK, 0) < 0) {
        THROW_ERROR("failed to faccessat file with abs path and invalid dirfd");
    }
    if (remove_file(file_path) < 0) {
        return -1;
    }
    if (faccessat(AT_FDCWD, file_path, F_OK, 0) >= 0 || errno != ENOENT) {
        THROW_ERROR("failed to faccessat file after unlink");
    }
    return 0;
}

static int __test_faccessat_with_dirfd(const char *file_path) {
    char dir_buf[PATH_MAX] = { 0 };
    char base_buf[PATH_MAX] = { 0 };
    char *dir_name, *file_name;
    int dirfd;

    if (fs_split_path(file_path, dir_buf, &dir_name, base_buf, &file_name) < 0) {
        THROW_ERROR("failed to split path");
    }
    dirfd = open(dir_name, O_RDONLY);
    if (dirfd < 0) {
        THROW_ERROR("failed to open dir");
    }
    if (faccessat(dirfd, file_name, F_OK, 0) < 0) {
        close(dirfd);
        THROW_ERROR("failed to faccessat file with dirfd");
    }
    if (remove_file(file_path) < 0) {
        close(dirfd);
        return -1;
    }
    if (faccessat(dirfd, file_name, F_OK, 0) >= 0 || errno != ENOENT) {
        close(dirfd);
        THROW_ERROR("failed to faccessat file after unlink");
    }
    close(dirfd);
    return 0;
}

typedef int(*test_access_func_t)(const char *);

static int test_access_framework(test_access_func_t fn) {
    const char *file_path = "/root/test_filesystem_access.txt";
    mode_t mode = 00666;

    if (create_file(file_path, mode) < 0) {
        return -1;
    }
    if (fn(file_path) < 0) {
        return -1;
    }
    return 0;
}

static int test_access() {
    return test_access_framework(__test_access);
}

static int test_faccessat_with_abs_path() {
    return test_access_framework(__test_faccessat_with_abs_path);
}

static int test_faccessat_with_dirfd() {
    return test_access_framework(__test_faccessat_with_dirfd);
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_access),
    TEST_CASE(test_faccessat_with_abs_path),
    TEST_CASE(test_faccessat_with_dirfd),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
