#define _GNU_SOURCE
#include <sys/types.h>
#include <sys/vfs.h>
#include <fcntl.h>
#include <limits.h>
#include <stdlib.h>
#include <errno.h>
#include "test_fs.h"

// ============================================================================
// Helper variable and function
// ============================================================================

// Contains the name that was used to invoke the calling program
extern char *program_invocation_short_name;

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

static int test_read_from_procfs(const char *proc_inode) {
    char buf[1024] = { 0 };
    int len;

    int fd = open(proc_inode, O_RDONLY);
    if (fd < 0) {
        THROW_ERROR("failed to open file: %s", proc_inode);
    }
    do {
        len = read(fd, buf, sizeof(buf));
        if (len < 0) {
            THROW_ERROR("failed to read: %s", proc_inode);
        }
    } while (len == sizeof(buf));
    close(fd);
    return 0;
}


// ============================================================================
// Test cases for procfs
// ============================================================================

static int test_readlink_from_proc_self_exe() {
    char exe_buf[PATH_MAX] = { 0 };
    char absolute_path[PATH_MAX] = { 0 };
    const char *proc_exe = "/proc/self/exe";

    int n = snprintf(absolute_path, sizeof(absolute_path), "/bin/%s",
                     program_invocation_short_name);
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

    int n = snprintf(absolute_path, sizeof(absolute_path), "/bin/%s",
                     program_invocation_short_name);
    if (n < 0) {
        THROW_ERROR("failed to call snprintf");
    }
    char read_buf[PATH_MAX] = { 0 };
    int fd = open(proc_cmdline, O_RDONLY);
    size_t len = read(fd, read_buf, sizeof(read_buf));
    if (len != strlen(absolute_path) + 1) {
        THROW_ERROR("failed check the return value of reading from %s", proc_cmdline);
    }
    if (read_buf[strlen(absolute_path)] != '\0') {
        THROW_ERROR("failed check the buffer of reading from %s", proc_cmdline);
    }
    if (strcmp(absolute_path, read_buf) != 0) {
        THROW_ERROR("failed to check result in %s", proc_cmdline);
    }
    close(fd);
    return 0;
}

static int test_read_from_proc_self_comm() {
    // The name can be up to 16 bytes long, including the terminating null byte.
    char comm_name[16] = { 0 };
    const char *proc_comm = "/proc/self/comm";

    if (snprintf(comm_name, sizeof(comm_name), "%s", program_invocation_short_name) < 0) {
        THROW_ERROR("failed to call snprintf");
    }
    // The last byte shoud be '\n'
    int end_idx = strlen(comm_name);
    comm_name[end_idx] = '\n';
    if (fs_check_file_content(proc_comm, comm_name) < 0) {
        THROW_ERROR("failed to check result in %s", proc_comm);
    }

    return 0;
}

static int test_read_from_proc_self_stat() {
    const char *proc_self_stat = "/proc/self/stat";
    FILE *fp = fopen(proc_self_stat, "r");
    if (fp == NULL) {
        THROW_ERROR("failed to fopen: %s", proc_self_stat);
    }

    int pid, ppid, pgrp;
    char comm[32] = { 0 };
    char state[32] = { 0 };
    int ret = fscanf(fp, "%d %s %s %d %d", &pid, comm, state, &ppid, &pgrp);
    if (ret != 5) {
        THROW_ERROR("failed to parse the first 5 items");
    }
    if (pid != getpid()) {
        THROW_ERROR("failed to check the result in %s", proc_self_stat);
    }
    printf("cat %s with the first 5 items:\n%d %s %s %d %d\n", proc_self_stat, pid, comm,
           state, ppid, pgrp);

    fclose(fp);
    return 0;
}

static int test_read_from_proc_meminfo() {
    const char *proc_meminfo = "/proc/meminfo";

    if (test_read_from_procfs(proc_meminfo) < 0) {
        THROW_ERROR("failed to read the meminfo");
    }
    return 0;
}

static int test_read_from_proc_cpuinfo() {
    const char *proc_cpuinfo = "/proc/cpuinfo";

    if (test_read_from_procfs(proc_cpuinfo) < 0) {
        THROW_ERROR("failed to read the cpuinfo");
    }
    return 0;
}

#define PROC_SUPER_MAGIC 0x9fa0
static int test_statfs() {
    const char *file_path = "/proc/cpuinfo";
    struct statfs statfs_buf;
    int ret;

    ret = statfs(file_path, &statfs_buf);
    if (ret < 0) {
        THROW_ERROR("failed to statfs the file");
    }
    if (statfs_buf.f_type != PROC_SUPER_MAGIC) {
        THROW_ERROR("failed to check the f_type");
    }
    return 0;
}

static int test_readdir_root() {
    const char *root = "/proc";
    char pid[NAME_MAX] = { 0 };
    snprintf(pid, sizeof(pid), "%d", getpid());
    char expected_entries[4][NAME_MAX] = {
        "self",
        "meminfo",
        "cpuinfo",
        { *pid },
    };

    if (check_readdir_with_expected_entries(root, expected_entries, 4) < 0) {
        THROW_ERROR("failed to test readdir %s", root);
    }

    return 0;
}

static int test_readdir_self() {
    const char *self = "/proc/self";
    char expected_entries[6][NAME_MAX] = {
        "exe",
        "cwd",
        "root",
        "fd",
        "comm",
        "cmdline",
    };

    if (check_readdir_with_expected_entries(self, expected_entries, 6) < 0) {
        THROW_ERROR("failed to test readdir %s", self);
    }

    return 0;
}

static int test_readdir_self_fd() {
    const char *self_fd = "/proc/self/fd";
    char expected_entries[3][NAME_MAX] = {
        "0",
        "1",
        "2",
    };

    if (check_readdir_with_expected_entries(self_fd, expected_entries, 3) < 0) {
        THROW_ERROR("failed to test readdir %s", self_fd);
    }

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
    TEST_CASE(test_read_from_proc_self_comm),
    TEST_CASE(test_read_from_proc_self_stat),
    TEST_CASE(test_read_from_proc_meminfo),
    TEST_CASE(test_read_from_proc_cpuinfo),
    TEST_CASE(test_statfs),
    TEST_CASE(test_readdir_root),
    TEST_CASE(test_readdir_self),
    TEST_CASE(test_readdir_self_fd),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
