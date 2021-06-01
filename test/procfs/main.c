#include <sys/types.h>
#include <fcntl.h>
#include <limits.h>
#include <stdlib.h>
#include <errno.h>
#include "test_fs.h"

// ============================================================================
// Helper variable and function
// ============================================================================

const char **g_argv;

static int test_readlink_from_procfs(const char *proc_inode, char *buf, int buf_size,
                                     const char *expected_target) {
    int n = readlink(proc_inode, buf, buf_size);
    if (n < 0) {
        THROW_ERROR("failed to readlink from %s", proc_inode);
    } else if (n != strlen(expected_target)) {
        THROW_ERROR("readlink from %s length is wrong", proc_inode);
    }
    if (strncmp(buf, expected_target, n) != 0) {
        THROW_ERROR("check the result from %s failed", proc_inode);
    }

    return 0;
}

// ============================================================================
// Test cases for procfs
// ============================================================================

static int test_readlink_from_proc_self_exe() {
    char exe_buf[PATH_MAX] = { 0 };
    char absolute_path[PATH_MAX] = { 0 };
    const char *proc_exe = "/proc/self/exe";

    int n = snprintf(absolute_path, sizeof(absolute_path), "/bin/%s", *g_argv);
    if (n < 0) {
        THROW_ERROR("failed to call snprintf");
    }
    if (test_readlink_from_procfs(proc_exe, exe_buf, PATH_MAX, absolute_path) < 0) {
        THROW_ERROR("failed to call test_readlink_from_procfs");
    }

    return 0;
}

static int test_readlink_from_proc_self_cwd() {
    char cwd_buf[PATH_MAX] = { 0 };
    const char *proc_cwd = "/proc/self/cwd";

    if (test_readlink_from_procfs(proc_cwd, cwd_buf, PATH_MAX, "/") < 0) {
        THROW_ERROR("failed to call test_readlink_from_procfs");
    }
    if (chdir("/bin") < 0) {
        THROW_ERROR("failed to chdir");
    }
    if (test_readlink_from_procfs(proc_cwd, cwd_buf, PATH_MAX, "/bin") < 0) {
        THROW_ERROR("failed to call test_readlink_from_procfs after chdir");
    }
    if (chdir("/") < 0) {
        THROW_ERROR("failed to chdir");
    }

    return 0;
}

static int test_readlink_from_proc_self_root() {
    char root_buf[PATH_MAX] = { 0 };
    const char *proc_root = "/proc/self/root";

    if (test_readlink_from_procfs(proc_root, root_buf, PATH_MAX, "/") < 0) {
        THROW_ERROR("failed to call test_readlink_from_procfs");
    }
    return 0;
}

static int test_create_and_unlink_file_from_proc_self_root() {
    const char *proc_root_file = "/proc/self/root/test_file";
    int fd = open(proc_root_file, O_RDONLY | O_CREAT | O_TRUNC, 00666);
    if (fd < 0) {
        THROW_ERROR("failed to create a file");
    }
    close(fd);
    if (unlink(proc_root_file) < 0) {
        THROW_ERROR("failed to unlink the created file");
    }
    return 0;
}

static int test_read_from_proc_self_cmdline() {
    char absolute_path[PATH_MAX] = { 0 };
    const char *proc_cmdline = "/proc/self/cmdline";

    int n = snprintf(absolute_path, sizeof(absolute_path), "/bin/%s", *g_argv);
    if (n < 0) {
        THROW_ERROR("failed to call snprintf");
    }
    if (fs_check_file_content(proc_cmdline, absolute_path) < 0) {
        THROW_ERROR("failed to check result in %s", proc_cmdline);
    }

    return 0;
}

static int test_read_from_proc_meminfo() {
    char meminfo[1024] = { 0 };
    const char *proc_meminfo = "/proc/meminfo";

    int fd = open(proc_meminfo, O_RDONLY);
    if (fd < 0) {
        THROW_ERROR("failed to open file: %s", proc_meminfo);
    }
    if (read(fd, meminfo, sizeof(meminfo)) < 0) {
        THROW_ERROR("failed to read the meminfo");
    }
    close(fd);

    return 0;
}

static int test_read_from_proc_cpuinfo() {
    char cpuinfo[1024] = { 0 };
    const char *proc_cpuinfo = "/proc/cpuinfo";
    int len;

    int fd = open(proc_cpuinfo, O_RDONLY);
    if (fd < 0) {
        THROW_ERROR("failed to open file: %s", proc_cpuinfo);
    }
    do {
        len = read(fd, cpuinfo, sizeof(cpuinfo));
        if (len < 0) {
            THROW_ERROR("failed to read the cpuinfo");
        } else if (len < sizeof(cpuinfo)) {
            break;
        }
    } while (len == sizeof(cpuinfo));
    close(fd);

    return 0;
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_readlink_from_proc_self_exe),
    TEST_CASE(test_readlink_from_proc_self_cwd),
    TEST_CASE(test_readlink_from_proc_self_root),
    TEST_CASE(test_create_and_unlink_file_from_proc_self_root),
    TEST_CASE(test_read_from_proc_self_cmdline),
    TEST_CASE(test_read_from_proc_meminfo),
    TEST_CASE(test_read_from_proc_cpuinfo),
};

int main(int argc, const char *argv[]) {
    g_argv = argv;
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
