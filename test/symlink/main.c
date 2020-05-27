#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <libgen.h>
#include <limits.h>
#include <unistd.h>
#include <string.h>
#include <stdio.h>
#include <stdlib.h>
#include "test.h"

// ============================================================================
// Helper variable and function
// ============================================================================
const char **g_argv;

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
    char buf[128] = { 0 };
    char dirc[128] = { 0 };
    char basec[128] = { 0 };
    char *dir_name, *file_name, *res;
    int ret;

    if (snprintf(dirc, sizeof(dirc), "%s", file_path) < 0 ||
            snprintf(basec, sizeof(dirc), "%s", file_path) < 0) {
        THROW_ERROR("failed to copy file path");
    }
    dir_name = dirname(dirc);
    file_name = basename(basec);
    ret = chdir(dir_name);
    if (ret < 0) {
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


static int test_readlink_from_proc_self_exe() {
    char exe_buf[128] = { 0 };
    char absolute_path[128] = { 0 };
    const char *proc_exe = "/proc/self/exe";
    ssize_t n;

    n = snprintf(absolute_path, sizeof(absolute_path), "/bin/%s", *g_argv);
    if (n < 0) {
        THROW_ERROR("failed to call snprintf");
    }
    n = readlink(proc_exe, exe_buf, sizeof(exe_buf));
    if (n < 0) {
        THROW_ERROR("failed to readlink from %s", proc_exe);
    } else if (n != strlen(absolute_path)) {
        THROW_ERROR("readlink from %s length is wrong", proc_exe);
    }
    if (strncmp(exe_buf, absolute_path, n) != 0) {
        THROW_ERROR("check the absolute path from %s failed", proc_exe);
    }

    return 0;
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_readlink_from_proc_self_fd),
    TEST_CASE(test_realpath),
    TEST_CASE(test_readlink_from_proc_self_exe),
};

int main(int argc, const char *argv[]) {
    g_argv = argv;
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
