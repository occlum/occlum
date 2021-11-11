#include <sys/vfs.h>
#include <sys/stat.h>
#include <errno.h>
#include <fcntl.h>
#include <unistd.h>
#include "test_fs.h"

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
// Test cases for statfs
// ============================================================================

static int __test_statfs(const char *file_path, unsigned long expected_type) {
    struct statfs statfs_buf;
    int ret;

    ret = statfs(file_path, &statfs_buf);
    if (ret < 0) {
        THROW_ERROR("failed to statfs the file");
    }
    if (statfs_buf.f_type != expected_type) {
        THROW_ERROR("failed to check the f_type");
    }
    return 0;
}

static int __test_fstatfs(const char *file_path, unsigned long expected_type) {
    struct statfs statfs_buf;
    int fd, ret;
    int flags = O_RDONLY;

    fd = open(file_path, flags);
    if (fd < 0) {
        THROW_ERROR("failed to open file");
    }
    ret = fstatfs(fd, &statfs_buf);
    if (ret < 0) {
        THROW_ERROR("failed to fstatfs the file");
    }
    if (statfs_buf.f_type != expected_type) {
        THROW_ERROR("failed to check the f_type");
    }
    close(fd);
    return 0;
}

typedef int(*test_statfs_func_t)(const char *, unsigned long);

static int test_statfs_framework(test_statfs_func_t fn, const char *file_path,
                                 unsigned long expected_type) {
    if (create_file(file_path) < 0) {
        return -1;
    }
    if (fn(file_path, expected_type) < 0) {
        return -1;
    }
    if (remove_file(file_path) < 0) {
        return -1;
    }
    return 0;
}

#define UNIONFS_MAGIC  0x2f8dbe2f
#define TMPFS_MAGIC    0x01021994

static int test_statfs_on_root() {
    const char *file_path = "/root/test_fs_statfs.txt";
    unsigned long expected_type = UNIONFS_MAGIC;
    return test_statfs_framework(__test_statfs, file_path, expected_type) +
           test_statfs_framework(__test_fstatfs, file_path, expected_type);
}

static int test_statfs_on_dev_shm() {
    const char *file_path = "/dev/shm/test_fs_statfs.txt";
    unsigned long expected_type = TMPFS_MAGIC;
    return test_statfs_framework(__test_statfs, file_path, expected_type) +
           test_statfs_framework(__test_fstatfs, file_path, expected_type);
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_statfs_on_root),
    TEST_CASE(test_statfs_on_dev_shm),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
