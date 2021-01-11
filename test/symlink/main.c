#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <limits.h>
#include <stdlib.h>
#include <errno.h>
#include "test_fs.h"

// ============================================================================
// Helper variable and function
// ============================================================================

static ssize_t get_path_by_fd(int fd, char *buf, ssize_t buf_len) {
    char proc_fd[64] = { 0 };
    int n;

    n = snprintf(proc_fd, sizeof(proc_fd), "/proc/self/fd/%d", fd);
    if (n < 0) {
        THROW_ERROR("failed to call snprintf for %d", fd);
    }

    return readlink(proc_fd, buf, buf_len);
}

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
// Test cases for readlink
// ============================================================================

static int __test_readlink_from_proc_self_fd(const char *file_path) {
    char buf[128] = { 0 };
    int fd;
    ssize_t n;

    fd = open(file_path, O_RDONLY);
    if (fd < 0) {
        THROW_ERROR("failed to open `%s` for read", file_path);
    }
    n = get_path_by_fd(fd, buf, sizeof(buf));
    close(fd);
    if (n < 0) {
        THROW_ERROR("failed to readlink for `%s`", file_path);
    }
    if (n != strlen(file_path)) {
        THROW_ERROR("readlink for `%s` length is wrong", file_path);
    }
    if (strncmp(buf, file_path, n) != 0) {
        THROW_ERROR("check the path for `%s` failed", file_path);
    }

    return 0;
}

static int __test_realpath(const char *file_path) {
    char buf[PATH_MAX] = { 0 };
    char dir_buf[PATH_MAX] = { 0 };
    char base_buf[PATH_MAX] = { 0 };
    char *dir_name, *file_name, *res;

    if (fs_split_path(file_path, dir_buf, &dir_name, base_buf, &file_name) < 0) {
        THROW_ERROR("failed to split path");
    }
    if (chdir(dir_name) < 0) {
        THROW_ERROR("failed to chdir to %s", dir_name);
    }
    res = realpath(file_name, buf);
    if (res == NULL) {
        THROW_ERROR("failed to get the realpath for `%s`", file_name);
    }
    if (strlen(buf) != strlen(file_path)) {
        THROW_ERROR("realpath for '%s' length is wrong", file_name);
    }
    if (strncmp(buf, file_path, strlen(buf)) != 0) {
        THROW_ERROR("check the realpath for '%s' failed", file_name);
    }
    if (chdir("/") < 0) {
        THROW_ERROR("failed to chdir to '/'");
    }

    return 0;
}

static int __test_readlinkat(const char *file_path) {
    int dirfd;
    size_t n;
    char buf[128] = { 0 };
#define LINK_DIR "/root"
#define LINK_NAME "test_symlink.link"
    const char *link_path = LINK_DIR"/"LINK_NAME;
    if (symlink(file_path, link_path) < 0) {
        THROW_ERROR("failed to create symlink");
    }

    dirfd = open(LINK_DIR, O_RDONLY);
    if (dirfd < 0) {
        THROW_ERROR("failed to open dir");
    }
    n = readlinkat(dirfd, LINK_NAME, buf, sizeof(buf));
    if (n < 0) {
        THROW_ERROR("failed to readlinkat from %s", link_path);
    } else if (n != strlen(file_path)) {
        THROW_ERROR("readlink from %s length is wrong", link_path);
    }
    if (strncmp(buf, file_path, n) != 0) {
        THROW_ERROR("check the content from %s failed", link_path);
    }
    close(dirfd);
    if (remove_file(link_path) < 0) {
        THROW_ERROR("failed to delete link file");
    }
    return 0;
}

typedef int(*test_readlink_func_t)(const char *);

static int test_readlink_framework(test_readlink_func_t fn) {
    const char *file_path = "/root/test_filesystem_symlink.txt";

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

static int test_readlink_from_proc_self_fd() {
    return test_readlink_framework(__test_readlink_from_proc_self_fd);
}

static int test_realpath() {
    return test_readlink_framework(__test_realpath);
}

static int test_readlinkat() {
    return test_readlink_framework(__test_readlinkat);
}

// ============================================================================
// Test cases for symlink
// ============================================================================

static int __test_symlinkat(const char *target, const char *link_path) {
    char dir_buf[PATH_MAX] = { 0 };
    char base_buf[PATH_MAX] = { 0 };
    char *dir_name, *link_name;

    if (create_file(target) < 0) {
        THROW_ERROR("failed to create target file");
    }
    int fd = open(target, O_WRONLY);
    if (fd < 0) {
        THROW_ERROR("failed to open target to write");
    }
    char *write_str = "Hello World\n";
    if (write(fd, write_str, strlen(write_str)) <= 0) {
        THROW_ERROR("failed to write");
    }
    close(fd);

    if (fs_split_path(link_path, dir_buf, &dir_name, base_buf, &link_name) < 0) {
        THROW_ERROR("failed to split path");
    }
    int dirfd = open(dir_name, O_RDONLY);
    if (dirfd < 0) {
        THROW_ERROR("failed to open dir");
    }
    if (symlinkat(target, dirfd, link_name) < 0) {
        THROW_ERROR("failed to create symlink");
    }
    close(dirfd);

    fd = open(link_path, O_RDONLY);
    if (fd < 0) {
        THROW_ERROR("failed to open link file to read");
    }
    char read_buf[128] = { 0 };
    if (read(fd, read_buf, sizeof(read_buf)) != strlen(write_str)) {
        THROW_ERROR("failed to read");
    }
    if (strcmp(write_str, read_buf) != 0) {
        THROW_ERROR("the message read from the file is not as it was written");
    }
    close(fd);

    if (remove_file(target) < 0) {
        THROW_ERROR("failed to delete target file");
    }
    return 0;
}


static int __test_symlink(const char *target, const char *link_path) {
    char dir_buf[PATH_MAX] = { 0 };
    char *dir_name;
    char target_path[PATH_MAX * 2] = { 0 };

    if (target[0] == '/') {
        snprintf(target_path, sizeof(target_path), "%s", target);
    } else {
        // If `target` is not an absolute path,
        // it must be a path relative to the directory of the `link_path`
        if (fs_split_path(link_path, dir_buf, &dir_name, NULL, NULL) < 0) {
            THROW_ERROR("failed to split path");
        }
        snprintf(target_path, sizeof(target_path), "%s/%s", dir_name, target);
    }
    if (create_file(target_path) < 0) {
        THROW_ERROR("failed to create target file");
    }

    int fd = open(target_path, O_WRONLY);
    if (fd < 0) {
        THROW_ERROR("failed to open target to write");
    }
    char *write_str = "Hello World\n";
    if (write(fd, write_str, strlen(write_str)) <= 0) {
        THROW_ERROR("failed to write");
    }
    close(fd);

    if (symlink(target, link_path) < 0) {
        THROW_ERROR("failed to create symlink");
    }

    fd = open(link_path, O_RDONLY | O_NOFOLLOW);
    if (fd >= 0 || errno != ELOOP) {
        THROW_ERROR("failed to check open file with O_NOFOLLOW flags");
    }

    fd = open(link_path, O_RDONLY);
    if (fd < 0) {
        THROW_ERROR("failed to open link file to read");
    }
    char read_buf[128] = { 0 };
    if (read(fd, read_buf, sizeof(read_buf)) != strlen(write_str)) {
        THROW_ERROR("failed to read");
    }
    if (strcmp(write_str, read_buf) != 0) {
        THROW_ERROR("the message read from the file is not as it was written");
    }
    close(fd);

    char readlink_buf[256] = { 0 };
    if (readlink(link_path, readlink_buf, sizeof(readlink_buf)) < 0) {
        THROW_ERROR("readlink failed");
    }
    if (strcmp(target, readlink_buf) != 0) {
        THROW_ERROR("check readlink result failed");
    }

    if (remove_file(target_path) < 0) {
        THROW_ERROR("failed to delete target file");
    }

    return 0;
}

static int __test_create_file_from_symlink(const char *target, const char *link_path) {
    char dir_buf[PATH_MAX] = { 0 };
    char *dir_name;
    char target_path[PATH_MAX * 2] = { 0 };

    if (target[0] == '/') {
        snprintf(target_path, sizeof(target_path), "%s", target);
    } else {
        // If `target` is not an absolute path,
        // it must be a path relative to the directory of the `link_path`
        if (fs_split_path(link_path, dir_buf, &dir_name, NULL, NULL) < 0) {
            THROW_ERROR("failed to split path");
        }
        snprintf(target_path, sizeof(target_path), "%s/%s", dir_name, target);
    }

    if (symlink(target, link_path) < 0) {
        THROW_ERROR("failed to create symlink");
    }

    int fd = open(link_path, O_RDONLY, 00666);
    if (fd >= 0 || errno != ENOENT) {
        THROW_ERROR("failed to check open a dangling symbolic link");
    }

    if (create_file(link_path) < 0) {
        THROW_ERROR("failed to create link file");
    }
    struct stat stat_buf;
    if (stat(target_path, &stat_buf) < 0) {
        THROW_ERROR("failed to stat the target file");
    }

    if (remove_file(target_path) < 0) {
        THROW_ERROR("failed to delete target file");
    }

    return 0;
}

typedef int(*test_symlink_func_t)(const char *, const char *);

static int test_symlink_framework(test_symlink_func_t fn, const char *target,
                                  const char *link) {
    if (fn(target, link) < 0) {
        return -1;
    }
    if (remove_file(link) < 0) {
        return -1;
    }

    return 0;
}

static int test_symlinkat() {
    char *target = "/root/test_symlink.file";
    char *link = "/root/test_symlink.link";
    return test_symlink_framework(__test_symlinkat, target, link);
}

static int test_symlink_to_absolute_target() {
    char *target = "/root/test_symlink.file";
    char *link = "/root/test_symlink.link";
    return test_symlink_framework(__test_symlink, target, link);
}

static int test_symlink_to_relative_target() {
    char *target = "./test_symlink.file";
    char *link = "/root/test_symlink.link";
    if (test_symlink_framework(__test_symlink, target, link) < 0) {
        return -1;
    }
    target = "../root/test_symlink.file";
    if (test_symlink_framework(__test_symlink, target, link) < 0) {
        return -1;
    }
    return 0;
}

static int test_symlink_from_ramfs() {
    char *target = "/root/test_symlink.file";
    char *link = "/tmp/test_symlink.link";
    return test_symlink_framework(__test_symlink, target, link);
}

static int test_symlink_to_ramfs() {
    char *target = "/tmp/test_symlink.file";
    char *link = "/root/test_symlink.link";
    return test_symlink_framework(__test_symlink, target, link);
}

static int test_symlink_with_empty_target_or_link_path() {
    char *target = "/root/test_symlink.file";
    char *link_path = "/root/test_symlink.link";

    int ret = symlink("", link_path);
    if (ret >= 0 || errno != ENOENT) {
        THROW_ERROR("failed to check symlink with empty target");
    }
    ret = symlink(target, "");
    if (ret >= 0 || errno != ENOENT) {
        THROW_ERROR("failed to check symlink with empty linkpath");
    }
    return 0;
}

static int test_create_file_from_symlink_to_absolute_target() {
    char *target = "/root/test_symlink.file";
    char *link = "/root/test_symlink.link";
    return test_symlink_framework(__test_create_file_from_symlink, target, link);
}

static int test_create_file_from_symlink_to_relative_target() {
    char *target = "test_symlink.file";
    char *link = "/root/test_symlink.link";
    if (test_symlink_framework(__test_create_file_from_symlink, target, link) < 0) {
        return -1;
    }
    target = "../root/test_symlink.file";
    if (test_symlink_framework(__test_create_file_from_symlink, target, link) < 0) {
        return -1;
    }
    return 0;
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_readlink_from_proc_self_fd),
    TEST_CASE(test_realpath),
    TEST_CASE(test_readlinkat),
    TEST_CASE(test_symlinkat),
    TEST_CASE(test_symlink_to_absolute_target),
    TEST_CASE(test_symlink_to_relative_target),
    TEST_CASE(test_symlink_from_ramfs),
    TEST_CASE(test_symlink_to_ramfs),
    TEST_CASE(test_symlink_with_empty_target_or_link_path),
    TEST_CASE(test_create_file_from_symlink_to_absolute_target),
    TEST_CASE(test_create_file_from_symlink_to_relative_target),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
