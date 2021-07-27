#include <sys/stat.h>
#include <sys/syscall.h>
#include <errno.h>
#include <fcntl.h>
#include "test_fs.h"

// ============================================================================
// Helper function
// ============================================================================

static int create_dir(const char *dir_path) {
    int ret;

    ret = mkdir(dir_path, 00775);
    if (ret < 0) {
        THROW_ERROR("failed to create the dir");
    }
    return 0;
}

static int remove_dir(const char *dir_path) {
    int ret;

    ret = rmdir(dir_path);
    if (ret < 0) {
        THROW_ERROR("failed to remove the created dir");
    }
    return 0;
}

// ============================================================================
// Test cases for mkdir
// ============================================================================

static int __test_mkdir(const char *dir_path) {
    struct stat stat_buf;
    mode_t mode = 00775;

    if (mkdir(dir_path, mode) < 0) {
        THROW_ERROR("failed to mkdir");
    }
    if (stat(dir_path, &stat_buf) < 0) {
        THROW_ERROR("failed to stat dir");
    }
    if (!S_ISDIR(stat_buf.st_mode)) {
        THROW_ERROR("failed to check if it is dir");
    }
    return 0;
}

static int __test_mkdirat(const char *dir_path) {
    struct stat stat_buf;
    mode_t mode = 00775;
    char dir_buf[PATH_MAX] = { 0 };
    char base_buf[PATH_MAX] = { 0 };
    char *dir_name, *last_name;
    int dirfd;

    if (fs_split_path(dir_path, dir_buf, &dir_name, base_buf, &last_name) < 0) {
        THROW_ERROR("failed to split path");
    }
    dirfd = open(dir_name, O_RDONLY);
    if (dirfd < 0) {
        THROW_ERROR("failed to open dir");
    }
    if (mkdirat(dirfd, last_name, mode) < 0) {
        THROW_ERROR("failed to mkdirat dir with dirfd");
    }
    close(dirfd);

    if (stat(dir_path, &stat_buf) < 0) {
        THROW_ERROR("failed to stat dir");
    }
    if (!S_ISDIR(stat_buf.st_mode)) {
        THROW_ERROR("failed to check if it is dir");
    }
    return 0;
}

typedef int(*test_mkdir_func_t)(const char *);

static int test_mkdir_framework(test_mkdir_func_t fn) {
    const char *dir_path = "/root/test_filesystem_mkdir";

    if (fn(dir_path) < 0) {
        return -1;
    }
    if (remove_dir(dir_path) < 0) {
        return -1;
    }
    return 0;
}

static int test_mkdir() {
    return test_mkdir_framework(__test_mkdir);
}

static int test_mkdirat() {
    return test_mkdir_framework(__test_mkdirat);
}

// ============================================================================
// Test cases for chdir
// ============================================================================

static int __test_chdir(const char *dir_path) {
    char buf[128] = { 0 };
    char *cwd;

    if (chdir(dir_path) < 0) {
        THROW_ERROR("failed to chdir");
    }
    cwd = getcwd(buf, sizeof(buf));
    if (cwd != buf) {
        THROW_ERROR("failed to getcwd");
    }
    if (strcmp(buf, dir_path)) {
        THROW_ERROR("the cwd is incorrect after chdir");
    }

    // Check getcwd via explicit syscall
    int ret = syscall(__NR_getcwd, buf, sizeof(buf));
    if (ret < 0) {
        THROW_ERROR("failed to call via explicit syscall");
    }
    if (ret != strlen(dir_path) + 1) {
        THROW_ERROR("failed to check the return value from kernel");
    }
    return 0;
}

static int test_chdir_framework(test_mkdir_func_t fn) {
    const char *dir_path = "/root/test_filesystem_chdir";

    if (create_dir(dir_path) < 0) {
        return -1;
    }
    if (fn(dir_path) < 0) {
        return -1;
    }
    if (remove_dir(dir_path) < 0) {
        return -1;
    }
    return 0;
}

static int test_chdir() {
    return test_chdir_framework(__test_chdir);
}

// ============================================================================
// Test cases for rmdir
// ============================================================================

static int __test_rmdir_via_unlinkat(const char *dir_path) {
    struct stat stat_buf;
    int ret;

    if (unlinkat(AT_FDCWD, dir_path, AT_REMOVEDIR) < 0) {
        THROW_ERROR("failed to remove dir");
    }

    ret = stat(dir_path, &stat_buf);
    if (!(ret < 0 && errno == ENOENT)) {
        THROW_ERROR("stat on \"%s\" should return ENOENT", dir_path);
    }
    return 0;
}

static int test_rmdir_framework(test_mkdir_func_t fn) {
    const char *dir_path = "/root/test_filesystem_rmdir";

    if (create_dir(dir_path) < 0) {
        return -1;
    }
    if (fn(dir_path) < 0) {
        return -1;
    }
    return 0;
}

static int test_rmdir_via_unlinkat() {
    return test_rmdir_framework(__test_rmdir_via_unlinkat);
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_mkdir),
    TEST_CASE(test_mkdirat),
    TEST_CASE(test_chdir),
    TEST_CASE(test_rmdir_via_unlinkat),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
