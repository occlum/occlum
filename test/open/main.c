#include <sys/stat.h>
#include <fcntl.h>
#include <errno.h>
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

static int __test_open_file_with_dir_flags(const char *file_path, int flags, int mode) {
    flags = O_DIRECTORY | O_RDWR | O_CREAT;
    int fd = open(file_path, flags, mode);
    if (fd < 0) {
        THROW_ERROR("failed to check creating file with O_DIRECTORY");
    }
    close(fd);

    fd = open(file_path, flags, mode);
    if (!(fd < 0 && errno == ENOTDIR)) {
        THROW_ERROR("open file with O_DIRECTORY should return ENOTDIR");
    }
    return 0;
}

static int __test_open_dir_with_write_flags(const char *file_path, int flags, int mode) {
    char dir_buf[PATH_MAX] = { 0 };
    char *dir_name;
    int fd;

    if (__test_open(file_path, flags, mode) < 0) {
        THROW_ERROR("failed to create file");
    }
    if (fs_split_path(file_path, dir_buf, &dir_name, NULL, NULL) < 0) {
        THROW_ERROR("failed to split path");
    }

    flags = O_WRONLY;
    fd = open(dir_name, flags, mode);
    if (!(fd < 0 && errno == EISDIR)) {
        THROW_ERROR("open dir with write flags should return EISDIR");
    }
    return 0;
}

static int __test_openat_with_abs_path(const char *file_path, int flags, int mode) {
    int fd = openat(AT_FDCWD, file_path, flags, mode);
    if (fd < 0) {
        THROW_ERROR("failed to openat a file with abs path");
    }
    close(fd);

    fd = openat(-1, file_path, flags, mode);
    if (fd < 0) {
        THROW_ERROR("failed to openat a file with abs path and invalid dirfd");
    }
    close(fd);
    return 0;
}

static int __test_openat_with_dirfd(const char *file_path, int flags, int mode) {
    char dir_buf[PATH_MAX] = { 0 };
    char base_buf[PATH_MAX] = { 0 };
    char *dir_name, *file_name;
    int dirfd, fd;

    if (fs_split_path(file_path, dir_buf, &dir_name, base_buf, &file_name) < 0) {
        THROW_ERROR("failed to split path");
    }
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

static int __test_creat(const char *file_path, int flags, int mode) {
    int fd = creat(file_path, mode);
    if (fd < 0) {
        THROW_ERROR("failed to creat a file");
    }
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

static int test_open_file_with_dir_flags() {
    return test_open_framework(__test_open_file_with_dir_flags);
}

static int test_open_dir_with_write_flags() {
    return test_open_framework(__test_open_dir_with_write_flags);
}

static int test_openat_with_abs_path() {
    return test_open_framework(__test_openat_with_abs_path);
}

static int test_openat_with_dirfd() {
    return test_open_framework(__test_openat_with_dirfd);
}

static int test_creat() {
    return test_open_framework(__test_creat);
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_open),
    TEST_CASE(test_open_file_with_dir_flags),
    TEST_CASE(test_open_dir_with_write_flags),
    TEST_CASE(test_openat_with_abs_path),
    TEST_CASE(test_openat_with_dirfd),
    TEST_CASE(test_creat),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
