#include <sys/stat.h>
#include <sys/mount.h>
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

static int write_read_file(const char *file_path) {
    char *write_str = "Hello World\n";
    int fd;

    fd = open(file_path, O_RDWR | O_CREAT | O_TRUNC, 00666);
    if (fd < 0) {
        THROW_ERROR("failed to open a file to write");
    }
    if (write(fd, write_str, strlen(write_str)) <= 0) {
        THROW_ERROR("failed to write");
    }
    close(fd);

    if (fs_check_file_content(file_path, write_str) < 0) {
        THROW_ERROR("failed to check file content");
    }

    if (remove_file(file_path) < 0) {
        THROW_ERROR("failed to remove: %s", file_path);
    }

    return 0;
}

static int create_mnt_dir(const char *mnt_dir) {
    struct stat stat_buf;
    mode_t mode = 00775;

    if (stat(mnt_dir, &stat_buf) == 0) {
        if (!S_ISDIR(stat_buf.st_mode)) {
            if (remove_file(mnt_dir) < 0) {
                THROW_ERROR("failed to remove: %s", mnt_dir);
            }
            if (mkdir(mnt_dir, mode) < 0) {
                THROW_ERROR("failed to mkdir: %s", mnt_dir);
            }
        }
    } else {
        if (mkdir(mnt_dir, mode) < 0) {
            THROW_ERROR("failed to mkdir: %s", mnt_dir);
        }
    }
    return 0;
}

// ============================================================================
// Test cases for file
// ============================================================================

/* TODO: enable it if SEFS is thread-safe
static int __test_mount_sefs(const char *mnt_dir) {
    if (create_mnt_dir(mnt_dir) < 0) {
        THROW_ERROR("failed to create sefs mnt dir");
    }

    if (mount("sefs", mnt_dir, "sefs", 0, "dir=./mnt_test/mnt_sefs") < 0) {
        THROW_ERROR("failed to mount sefs");
    }

    return 0;
}
*/

static int __test_mount_unionfs(const char *mnt_dir) {
    if (create_mnt_dir(mnt_dir) < 0) {
        THROW_ERROR("failed to create unionfs mnt dir");
    }

    if (mount("unionfs", mnt_dir, "unionfs", 0,
              "lowerdir=./mnt_test/mnt_unionfs/lower,upperdir=./mnt_test/mnt_unionfs/upper") < 0) {
        THROW_ERROR("failed to mount unionfs");
    }

    return 0;
}

static int __test_mount_hostfs(const char *mnt_dir) {
    if (create_mnt_dir(mnt_dir) < 0) {
        THROW_ERROR("failed to create hostfs mnt dir");
    }

    if (mount("hostfs", mnt_dir, "hostfs", 0, "dir=./mnt_test/mnt_hostfs") < 0) {
        THROW_ERROR("failed to mount hostfs");
    }

    return 0;
}

static int __test_mount_ramfs(const char *mnt_dir) {
    if (create_mnt_dir(mnt_dir) < 0) {
        THROW_ERROR("failed to create ramfs mnt dir");
    }

    if (mount("ramfs", mnt_dir, "ramfs", 0, NULL) < 0) {
        THROW_ERROR("failed to mount ramfs");
    }

    return 0;
}

typedef int(*test_mount_func_t)(const char *);

static int test_mount_framework(test_mount_func_t fn, const char *mnt_dir) {
    if (fn(mnt_dir) < 0) {
        return -1;
    }

    char file_path[PATH_MAX] = { 0 };
    snprintf(file_path, sizeof(file_path), "%s/test_write_read.txt", mnt_dir);
    if (write_read_file(file_path) < 0) {
        return -1;
    }

    return 0;
}

/* TODO: enable it if SEFS is thread-safe
static int test_mount_sefs() {
    const char *mnt_dir = "/mnt_sefs";
    return test_mount_framework(__test_mount_sefs, mnt_dir);
}
*/

static int test_mount_unionfs() {
    const char *mnt_dir = "/mnt_unionfs";
    return test_mount_framework(__test_mount_unionfs, mnt_dir);
}

static int test_mount_hostfs() {
    const char *mnt_dir = "/mnt_hostfs";
    return test_mount_framework(__test_mount_hostfs, mnt_dir);
}

static int test_mount_ramfs() {
    const char *mnt_dir = "/mnt_ramfs";
    return test_mount_framework(__test_mount_ramfs, mnt_dir);
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    //TEST_CASE(test_mount_sefs),
    TEST_CASE(test_mount_unionfs),
    TEST_CASE(test_mount_hostfs),
    TEST_CASE(test_mount_ramfs),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
