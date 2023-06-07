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

    return 0;
}

static int create_dir(const char *dir) {
    struct stat stat_buf;
    mode_t mode = 00775;

    if (stat(dir, &stat_buf) == 0) {
        if (!S_ISDIR(stat_buf.st_mode)) {
            if (remove_file(dir) < 0) {
                THROW_ERROR("failed to remove: %s", dir);
            }
            if (mkdir(dir, mode) < 0) {
                THROW_ERROR("failed to mkdir: %s", dir);
            }
        }
    } else {
        if (mkdir(dir, mode) < 0) {
            THROW_ERROR("failed to mkdir: %s", dir);
        }
    }
    return 0;
}

static int check_file_no_exists(const char *file_path) {
    struct stat stat_buf;

    int ret = stat(file_path, &stat_buf);
    if (!(ret < 0 && errno == ENOENT)) {
        THROW_ERROR("stat on \"%s\" should return ENOENT", file_path);
    }

    return 0;
}

// ============================================================================
// Test cases for mount
// ============================================================================

static int __test_mount_sefs(const char *mnt_dir) {
    if (create_dir(mnt_dir) < 0) {
        THROW_ERROR("failed to create sefs mnt dir");
    }

    if (mount("sefs", mnt_dir, "sefs", 0, "dir=./mnt_test/mnt_sefs") < 0) {
        THROW_ERROR("failed to mount sefs");
    }

    return 0;
}

static int __test_mount_unionfs(const char *mnt_dir) {
    if (create_dir(mnt_dir) < 0) {
        THROW_ERROR("failed to create unionfs mnt dir");
    }

    if (mount("unionfs", mnt_dir, "unionfs", 0,
              "lowerdir=./mnt_test/mnt_unionfs/lower,lowerfs=sefs,upperdir=./mnt_test/mnt_unionfs/upper,upperfs=async_sfs,sfssize=5GB,cachesize=128MB")
            < 0) {
        THROW_ERROR("failed to mount unionfs");
    }

    return 0;
}

static int __test_mount_hostfs(const char *mnt_dir) {
    if (create_dir(mnt_dir) < 0) {
        THROW_ERROR("failed to create hostfs mnt dir");
    }

    if (mount("hostfs", mnt_dir, "hostfs", 0, "dir=./mnt_test/mnt_hostfs") < 0) {
        THROW_ERROR("failed to mount hostfs");
    }

    return 0;
}

static int __test_mount_ramfs(const char *mnt_dir) {
    if (create_dir(mnt_dir) < 0) {
        THROW_ERROR("failed to create ramfs mnt dir");
    }

    if (mount("ramfs", mnt_dir, "ramfs", 0, NULL) < 0) {
        THROW_ERROR("failed to mount ramfs");
    }

    return 0;
}

typedef int(*test_mount_func_t)(const char *);

static int test_mount_framework(test_mount_func_t fn, const char *dir, bool mount) {
    if (fn(dir) < 0) {
        return -1;
    }

    char file_path[PATH_MAX] = { 0 };
    snprintf(file_path, sizeof(file_path), "%s/test_write_read.txt", dir);

    if (mount) {
        if (write_read_file(file_path) < 0) {
            THROW_ERROR("failed to RW files on mounted fs");
        }
    } else {
        if (check_file_no_exists(file_path) < 0) {
            THROW_ERROR("failed to check file exists after umount");
        }
    }

    return 0;
}

static int test_mount_sefs() {
    const char *mnt_dir = "/mnt_sefs";
    return test_mount_framework(__test_mount_sefs, mnt_dir, true);
}

static int test_mount_unionfs() {
    const char *mnt_dir = "/mnt_unionfs";
    return test_mount_framework(__test_mount_unionfs, mnt_dir, true);
}

static int test_mount_hostfs() {
    const char *mnt_dir = "/mnt_hostfs";
    return test_mount_framework(__test_mount_hostfs, mnt_dir, true);
}

static int test_mount_ramfs() {
    const char *mnt_dir = "/mnt_ramfs";
    return test_mount_framework(__test_mount_ramfs, mnt_dir, true);
}

// ============================================================================
// Test cases for umount
// ============================================================================

static int __test_umount_fs(const char *target) {
    int flags = MNT_EXPIRE | MNT_DETACH;
    int ret = umount2(target, flags);
    if (!(ret < 0 && errno == EINVAL)) {
        THROW_ERROR("failed to check invalid flags");
    }

    char subdir[PATH_MAX] = { 0 };
    snprintf(subdir, sizeof(subdir), "%s/subdir", target);
    if (create_dir(subdir) < 0) {
        THROW_ERROR("failed to create dir: %s", subdir);
    }
    ret = umount(subdir);
    if (!(ret < 0 && errno == EINVAL)) {
        THROW_ERROR("failed to check umount non-mountpoint");
    }

    if (umount(target) < 0) {
        THROW_ERROR("failed to umount fs on: %s", target);
    }

    return 0;
}

static int test_umount_sefs() {
    const char *target = "/mnt_sefs";
    return test_mount_framework(__test_umount_fs, target, false);
}

static int test_umount_unionfs() {
    const char *target = "/mnt_unionfs";
    return test_mount_framework(__test_umount_fs, target, false);
}

static int test_umount_hostfs() {
    const char *target = "/mnt_hostfs";
    return test_mount_framework(__test_umount_fs, target, false);
}

static int test_umount_ramfs() {
    const char *target = "/mnt_ramfs";
    return test_mount_framework(__test_umount_fs, target, false);
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    // TODO: enable it if SEFS is thread-safe
    //TEST_CASE(test_mount_sefs),
    //TEST_CASE(test_umount_sefs),
    TEST_CASE(test_mount_unionfs),
    TEST_CASE(test_umount_unionfs),
    TEST_CASE(test_mount_hostfs),
    TEST_CASE(test_umount_hostfs),
    TEST_CASE(test_mount_ramfs),
    TEST_CASE(test_umount_ramfs),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
