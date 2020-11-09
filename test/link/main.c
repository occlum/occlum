#define _GNU_SOURCE
#include <sys/stat.h>
#include <errno.h>
#include <fcntl.h>
#include "test_fs.h"

// ============================================================================
// Helper function
// ============================================================================

#define WRITE_MSG "Hello World"

static int create_and_write_file(const char *file_path) {
    int fd;
    int flags = O_WRONLY | O_CREAT | O_TRUNC;
    int mode = 00666;

    fd = open(file_path, flags, mode);
    if (fd < 0) {
        THROW_ERROR("failed to create a file");
    }
    if (write(fd, WRITE_MSG, strlen(WRITE_MSG)) <= 0) {
        THROW_ERROR("failed to write to the file");
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
// Test cases for link
// ============================================================================

static int __test_link_then_unlink(const char *old_path, const char *new_path) {
    struct stat stat_buf;
    int ret;

    if (link(old_path, new_path) < 0) {
        THROW_ERROR("failed to link file");
    }

    if (fs_check_file_content(new_path, WRITE_MSG) < 0) {
        THROW_ERROR("failed to check file content");
    }

    if (unlink(new_path) < 0) {
        THROW_ERROR("failed to unlink the link");
    }
    ret = stat(new_path, &stat_buf);
    if (!(ret < 0 && errno == ENOENT)) {
        THROW_ERROR("stat on \"%s\" should return ENOENT", new_path);
    }
    return 0;
}

static int __test_linkat_then_unlinkat(const char *old_path, const char *new_path) {
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

    if (linkat(old_dirfd, old_file_name, new_dirfd, new_file_name, 0) < 0) {
        THROW_ERROR("failed to linkat with dirfd");
    }
    close(old_dirfd);

    if (fs_check_file_content(new_path, WRITE_MSG) < 0) {
        THROW_ERROR("failed to check file content");
    }

    if (unlinkat(new_dirfd, new_file_name, 0) < 0) {
        THROW_ERROR("failed to unlinkat the link");
    }
    close(new_dirfd);
    ret = stat(new_path, &stat_buf);
    if (!(ret < 0 && errno == ENOENT)) {
        THROW_ERROR("stat on \"%s\" should return ENOENT", new_path);
    }
    return 0;
}

static int __test_linkat_with_empty_oldpath(const char *old_path, const char *new_path) {
    char new_dir_buf[PATH_MAX] = { 0 };
    char new_base_buf[PATH_MAX] = { 0 };
    char *new_dir_name, *new_file_name;
    int old_dirfd, new_dirfd, ret;

    old_dirfd = open(old_path, O_RDONLY);
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

    ret = linkat(old_dirfd, "", new_dirfd, new_file_name, 0);
    if (!(ret < 0 && errno == ENOENT)) {
        THROW_ERROR("linkat with empty oldpath should return ENOENT");
    }
    if (linkat(old_dirfd, "", new_dirfd, new_file_name, AT_EMPTY_PATH) < 0) {
        THROW_ERROR("failed to linkat with empty oldpath and AT_EMPTY_PATH flags");
    }
    close(old_dirfd);
    close(new_dirfd);

    if (fs_check_file_content(new_path, WRITE_MSG) < 0) {
        THROW_ERROR("failed to check file content");
    }

    if (unlink(new_path) < 0) {
        THROW_ERROR("failed to unlink the link");
    }
    return 0;
}

typedef int(*test_link_func_t)(const char *, const char *);

static int test_link_framework(test_link_func_t fn) {
    const char *old_path = "/root/test_filesystem_link_old.txt";
    const char *new_path = "/root/test_filesystem_link_new.txt";

    if (create_and_write_file(old_path) < 0) {
        return -1;
    }
    if (fn(old_path, new_path) < 0) {
        return -1;
    }
    if (remove_file(old_path) < 0) {
        return -1;
    }
    return 0;
}

static int test_link_then_unlink() {
    return test_link_framework(__test_link_then_unlink);
}

static int test_linkat_then_unlinkat() {
    return test_link_framework(__test_linkat_then_unlinkat);
}

static int test_linkat_with_empty_oldpath() {
    return test_link_framework(__test_linkat_with_empty_oldpath);
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_link_then_unlink),
    TEST_CASE(test_linkat_then_unlinkat),
    TEST_CASE(test_linkat_with_empty_oldpath),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
