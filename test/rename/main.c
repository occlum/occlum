#include <sys/stat.h>
#include <errno.h>
#include <fcntl.h>
#include <stdbool.h>
#include "test_fs.h"

// ============================================================================
// Helper function
// ============================================================================

#define WRITE_MSG "Hello World"

static int create_file_with_content(const char *file_path, const char *msg) {
    int fd;
    int flags = O_WRONLY | O_CREAT | O_TRUNC;
    int mode = 00666;

    fd = open(file_path, flags, mode);
    if (fd < 0) {
        THROW_ERROR("failed to create a file");
    }
    if (msg != NULL) {
        if (write(fd, msg, strlen(msg)) <= 0) {
            THROW_ERROR("failed to write to the file");
        }
    }
    close(fd);
    return 0;
}

// ============================================================================
// Test cases for rename
// ============================================================================

static int __test_rename(const char *old_path, const char *new_path) {
    struct stat stat_buf;
    int ret;

    if (rename(old_path, new_path) < 0) {
        THROW_ERROR("failed to rename file");
    }

    if (fs_check_file_content(new_path, WRITE_MSG) < 0) {
        THROW_ERROR("failed to check file content");
    }

    ret = stat(old_path, &stat_buf);
    if (!(ret < 0 && errno == ENOENT)) {
        THROW_ERROR("stat on old path should return ENOENT");
    }
    if (unlink(new_path) < 0) {
        THROW_ERROR("failed to remove the new file");
    }
    return 0;
}

static int __test_renameat(const char *old_path, const char *new_path) {
    struct stat stat_buf;
    char old_dir_buf[PATH_MAX] = { 0 };
    char old_base_buf[PATH_MAX] = { 0 };
    char new_dir_buf[PATH_MAX] = { 0 };
    char new_base_buf[PATH_MAX] = { 0 };
    char *old_dir_name, *old_file_name, *new_dir_name, *new_file_name;
    int old_dirfd, new_dirfd, ret;

    if (fs_split_path(old_path, old_dir_buf, &old_dir_name, old_base_buf,
                      &old_file_name) < 0) {
        THROW_ERROR("failed to split old path");
    }
    old_dirfd = open(old_dir_name, O_RDONLY);
    if (old_dirfd < 0) {
        THROW_ERROR("failed to open old dir");
    }
    if (fs_split_path(new_path, new_dir_buf, &new_dir_name, new_base_buf,
                      &new_file_name) < 0) {
        THROW_ERROR("failed to split new path");
    }
    new_dirfd = open(new_dir_name, O_RDONLY);
    if (new_dirfd < 0) {
        THROW_ERROR("failed to open new dir");
    }
    if (renameat(old_dirfd, old_file_name, new_dirfd, new_file_name) < 0) {
        THROW_ERROR("failed to rename with dirfd");
    }
    close(old_dirfd);
    close(new_dirfd);

    if (fs_check_file_content(new_path, WRITE_MSG) < 0) {
        THROW_ERROR("failed to check file content");
    }

    ret = stat(old_path, &stat_buf);
    if (!(ret < 0 && errno == ENOENT)) {
        THROW_ERROR("stat on old path should return ENOENT");
    }
    if (unlink(new_path) < 0) {
        THROW_ERROR("failed to remove the new file");
    }
    return 0;
}

typedef int(*test_rename_func_t)(const char *, const char *);

static int test_rename_framework(test_rename_func_t fn, bool target_exist) {
    const char *old_path = "/root/test_filesystem_rename_old.txt";
    const char *new_path = "/root/test_filesystem_rename_new.txt";

    if (create_file_with_content(old_path, WRITE_MSG) < 0) {
        THROW_ERROR("failed to create old file with content");
    }
    if (target_exist) {
        if (create_file_with_content(new_path, NULL) < 0) {
            THROW_ERROR("failed to create new file");
        }
    }
    if (fn(old_path, new_path) < 0) {
        return -1;
    }
    return 0;
}

static int test_rename() {
    return test_rename_framework(__test_rename, false);
}

static int test_rename_with_target_exist() {
    return test_rename_framework(__test_rename, true);
}

static int test_renameat() {
    return test_rename_framework(__test_renameat, false);
}

static int test_rename_dir() {
    const char *old_dir = "/root/test_old_dir";
    const char *new_dir = "/root/test_new_dir";
    const char *file_name = "test_file.txt";
    char file_buf[128] = { 0 };
    mode_t mode = 00775;
    struct stat stat_buf;
    int ret;

    if (mkdir(old_dir, mode) < 0) {
        THROW_ERROR("failed to mkdir old dir");
    }
    ret = snprintf(file_buf, sizeof(file_buf), "%s/%s", old_dir, file_name);
    if (ret >= sizeof(file_buf) || ret < 0) {
        THROW_ERROR("failed to copy file buf");
    }

    if (create_file_with_content(file_buf, WRITE_MSG) < 0) {
        THROW_ERROR("failed to create file in old dir");
    }

    if (rename(old_dir, new_dir) < 0) {
        THROW_ERROR("failed to rename dir");
    }

    ret = snprintf(file_buf, sizeof(file_buf), "%s/%s", new_dir, file_name);
    if (ret >= sizeof(file_buf) || ret < 0) {
        THROW_ERROR("failed to copy file buf");
    }

    if (fs_check_file_content(file_buf, WRITE_MSG) < 0) {
        THROW_ERROR("failed to check file content");
    }

    ret = stat(old_dir, &stat_buf);
    if (!(ret < 0 && errno == ENOENT)) {
        THROW_ERROR("stat on old dir should return ENOENT");
    }
    if (unlink(file_buf) < 0) {
        THROW_ERROR("failed to remove the file in new dir");
    }
    if (rmdir(new_dir) < 0) {
        THROW_ERROR("failed to remove the new dir");
    }
    return 0;
}

static int test_rename_dir_to_subdir() {
    const char *old_dir = "/root/test_old_dir";
    mode_t mode = 00775;
    int ret;

    char sub_dir[PATH_MAX] = { 0 };
    ret = snprintf(sub_dir, sizeof(sub_dir), "%s/test_new_dir", old_dir);
    if (ret >= sizeof(sub_dir) || ret < 0) {
        THROW_ERROR("failed to init new dir path");
    }

    if (mkdir(old_dir, mode) < 0) {
        THROW_ERROR("failed to mkdir");
    }

    ret = rename(old_dir, sub_dir);
    if (ret == 0 || errno != EINVAL) {
        THROW_ERROR("failed to check rename dir to subdir");
    }
    if (rmdir(old_dir) < 0) {
        THROW_ERROR("failed to rmdir");
    }
    return 0;
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    // TODO: test more corner cases
    TEST_CASE(test_rename),
    TEST_CASE(test_rename_with_target_exist),
    TEST_CASE(test_renameat),
    TEST_CASE(test_rename_dir),
    TEST_CASE(test_rename_dir_to_subdir),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
