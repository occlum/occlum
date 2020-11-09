#define _GNU_SOURCE
#include <sys/stat.h>
#include <errno.h>
#include <fcntl.h>
#include "test_fs.h"

// ============================================================================
// Helper function
// ============================================================================

static int create_file(const char *file_path) {
    int fd;
    int flags = O_RDONLY | O_CREAT | O_TRUNC;
    int mode = 00444;

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
// Test cases for chown
// ============================================================================

static int __test_chown(const char *file_path) {
    struct stat stat_buf;
    uid_t uid = 100;
    gid_t gid = 1000;
    int ret;

    ret = chown(file_path, uid, gid);
    if (ret < 0) {
        THROW_ERROR("failed to chown file");
    }
    ret = stat(file_path, &stat_buf);
    if (ret < 0) {
        THROW_ERROR("failed to stat file");
    }
    if (stat_buf.st_uid != uid || stat_buf.st_gid != gid) {
        THROW_ERROR("check chown result failed");
    }
    return 0;
}

static int __test_lchown(const char *file_path) {
    struct stat stat_buf;
    uid_t uid = 100;
    gid_t gid = 1000;
    int ret;

    ret = lchown(file_path, uid, gid);
    if (ret < 0) {
        THROW_ERROR("failed to lchown file");
    }
    ret = stat(file_path, &stat_buf);
    if (ret < 0) {
        THROW_ERROR("failed to stat file");
    }
    if (stat_buf.st_uid != uid || stat_buf.st_gid != gid) {
        THROW_ERROR("check lchown result failed");
    }
    return 0;
}

static int __test_fchown(const char *file_path) {
    struct stat stat_buf;
    uid_t uid = 100;
    gid_t gid = 1000;
    int fd, ret;

    fd = open(file_path, O_RDONLY);
    if (fd < 0) {
        THROW_ERROR("failed to open file");
    }
    ret = fchown(fd, uid, gid);
    if (ret < 0) {
        THROW_ERROR("failed to fchown file");
    }
    close(fd);
    ret = stat(file_path, &stat_buf);
    if (ret < 0) {
        THROW_ERROR("failed to stat file");
    }
    if (stat_buf.st_uid != uid || stat_buf.st_gid != gid) {
        THROW_ERROR("check fchown result failed");
    }
    return 0;
}

static int __test_fchownat(const char *file_path) {
    struct stat stat_buf;
    uid_t uid = 100;
    gid_t gid = 1000;
    char dir_buf[PATH_MAX] = { 0 };
    char base_buf[PATH_MAX] = { 0 };
    char *dir_name, *file_name;
    int dirfd, ret;

    if (fs_split_path(file_path, dir_buf, &dir_name, base_buf, &file_name) < 0) {
        THROW_ERROR("failed to split path");
    }
    dirfd = open(dir_name, O_RDONLY);
    if (dirfd < 0) {
        THROW_ERROR("failed to open dir");
    }
    ret = fchownat(dirfd, file_name, uid, gid, 0);
    if (ret < 0) {
        THROW_ERROR("failed to fchownat file with dirfd");
    }
    close(dirfd);
    ret = stat(file_path, &stat_buf);
    if (ret < 0) {
        THROW_ERROR("failed to stat file");
    }
    if (stat_buf.st_uid != uid || stat_buf.st_gid != gid) {
        THROW_ERROR("check fchownat result failed");
    }
    return 0;
}

static int __test_fchownat_with_empty_path(const char *file_path) {
    struct stat stat_buf;
    uid_t uid = 100;
    gid_t gid = 1000;
    char dir_buf[128] = { 0 };
    char *dir_name;
    int dirfd, ret;

    if (fs_split_path(file_path, dir_buf, &dir_name, NULL, NULL) < 0) {
        THROW_ERROR("failed to split path");
    }
    dirfd = open(dir_name, O_RDONLY);
    if (dirfd < 0) {
        THROW_ERROR("failed to open dir");
    }

    ret = fchownat(dirfd, "", uid, gid, 0);
    if (!(ret < 0 && errno == ENOENT)) {
        THROW_ERROR("fchownat with empty path should return ENOENT");
    }

    ret = fchownat(dirfd, "", uid, gid, AT_EMPTY_PATH);
    if (ret < 0) {
        THROW_ERROR("failed to fchownat with empty path");
    }
    close(dirfd);
    ret = stat(dir_name, &stat_buf);
    if (ret < 0) {
        THROW_ERROR("failed to stat dir");
    }
    if (stat_buf.st_uid != uid || stat_buf.st_gid != gid) {
        THROW_ERROR("check fchownat result failed");
    }
    return 0;
}

typedef int(*test_chown_func_t)(const char *);

static int test_chown_framework(test_chown_func_t fn) {
    const char *file_path = "/root/test_filesystem_chown.txt";

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

static int test_chown() {
    return test_chown_framework(__test_chown);
}

static int test_lchown() {
    return test_chown_framework(__test_lchown);
}

static int test_fchown() {
    return test_chown_framework(__test_fchown);
}

static int test_fchownat() {
    return test_chown_framework(__test_fchownat);
}

static int test_fchownat_with_empty_path() {
    return test_chown_framework(__test_fchownat_with_empty_path);
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_chown),
    TEST_CASE(test_lchown),
    TEST_CASE(test_fchown),
    TEST_CASE(test_fchownat),
    TEST_CASE(test_fchownat_with_empty_path),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
