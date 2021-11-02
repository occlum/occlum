#include <sys/stat.h>
#include <errno.h>
#include <fcntl.h>
#include <dirent.h>
#include <stdbool.h>
#include "test_fs.h"

// ============================================================================
// Helper function
// ============================================================================

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
// Test cases for hostfs
// ============================================================================

static int __test_write_read(const char *file_path) {
    char *write_str = "Write to hostfs successfully!";
    int fd;

    fd = open(file_path, O_WRONLY);
    if (fd < 0) {
        THROW_ERROR("failed to open a file to write");
    }
    if (write(fd, write_str, strlen(write_str)) <= 0) {
        THROW_ERROR("failed to write to the file");
    }
    close(fd);

    if (fs_check_file_content(file_path, write_str) < 0) {
        THROW_ERROR("failed to check file content");
    }
    return 0;
}

static int __test_write_fdatasync_read(const char *file_path) {
    char *write_str = "Write to hostfs and fdatasync successfully!";
    int fd;

    fd = open(file_path, O_WRONLY);
    if (fd < 0) {
        THROW_ERROR("failed to open a file to write");
    }
    if (write(fd, write_str, strlen(write_str)) <= 0) {
        THROW_ERROR("failed to write to the file");
    }
    if (fdatasync(fd) < 0) {
        THROW_ERROR("failed to sync data into file");
    }
    close(fd);

    if (fs_check_file_content(file_path, write_str) < 0) {
        THROW_ERROR("failed to check file content");
    }
    return 0;
}

static int __test_write_fsync_read(const char *file_path) {
    char *write_str = "Write to hostfs and fsync successfully!";
    int fd;

    fd = open(file_path, O_WRONLY);
    if (fd < 0) {
        THROW_ERROR("failed to open a file to write");
    }
    if (write(fd, write_str, strlen(write_str)) <= 0) {
        THROW_ERROR("failed to write to the file");
    }
    if (fsync(fd) < 0) {
        THROW_ERROR("failed to sync all into file");
    }
    close(fd);

    if (fs_check_file_content(file_path, write_str) < 0) {
        THROW_ERROR("failed to check file content");
    }
    return 0;
}

static int __test_rename(const char *file_path) {
    char *rename_path = "/host/hostfs_rename.txt";
    struct stat stat_buf;
    int ret;

    ret = rename(file_path, rename_path);
    if (ret < 0) {
        THROW_ERROR("failed to rename");
    }
    ret = stat(file_path, &stat_buf);
    if (!(ret < 0 && errno == ENOENT)) {
        THROW_ERROR("stat should return ENOENT");
    }
    ret = stat(rename_path, &stat_buf);
    if (ret < 0) {
        THROW_ERROR("failed to stat the file");
    }
    if (rename(rename_path, file_path) < 0) {
        THROW_ERROR("failed to rename back");
    }
    return 0;
}

static int __test_readdir(const char *file_path) {
    struct dirent *dp;
    DIR *dirp;
    char base_buf[PATH_MAX] = { 0 };
    char *base_name;
    bool found = false;

    if (fs_split_path(file_path, NULL, NULL, base_buf, &base_name) < 0) {
        THROW_ERROR("failed to split path");
    }
    dirp = opendir("/host");
    if (dirp == NULL) {
        THROW_ERROR("failed to open host directory");
    }
    while (1) {
        errno = 0;
        dp = readdir(dirp);
        if (dp == NULL) {
            if (errno != 0) {
                THROW_ERROR("faild to call readdir");
            }
            break;
        }
        if (strncmp(base_name, dp->d_name, strlen(base_name)) == 0) {
            found = true;
        }
    }
    if (!found) {
        THROW_ERROR("faild to read file entry");
    }
    closedir(dirp);
    return 0;
}

static int __test_truncate(const char *file_path) {
    off_t len = 256;
    if (truncate(file_path, len) < 0) {
        THROW_ERROR("failed to call truncate");
    }
    struct stat stat_buf;
    if (stat(file_path, &stat_buf) < 0) {
        THROW_ERROR("failed to stat file");
    }
    if (stat_buf.st_size != len) {
        THROW_ERROR("failed to check the len after truncate");
    }
    return 0;
}

typedef int(*test_hostfs_func_t)(const char *);

static int test_hostfs_framework(test_hostfs_func_t fn) {
    const char *file_path = "/host/hostfs_test.txt";

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

static int test_write_read() {
    return test_hostfs_framework(__test_write_read);
}

static int test_write_fdatasync_read() {
    return test_hostfs_framework(__test_write_fdatasync_read);
}

static int test_write_fsync_read() {
    return test_hostfs_framework(__test_write_fsync_read);
}

static int test_rename() {
    return test_hostfs_framework(__test_rename);
}

static int test_readdir() {
    return test_hostfs_framework(__test_readdir);
}

static int test_truncate() {
    return test_hostfs_framework(__test_truncate);
}

static int test_mkdir_then_rmdir() {
    const char *dir_path = "/host/hostfs_dir";
    struct stat stat_buf;

    if (mkdir(dir_path, 00775) < 0) {
        THROW_ERROR("failed to create the dir");
    }
    if (stat(dir_path, &stat_buf) < 0) {
        THROW_ERROR("failed to stat dir");
    }
    if (!S_ISDIR(stat_buf.st_mode)) {
        THROW_ERROR("failed to check if it is dir");
    }

    if (rmdir(dir_path) < 0) {
        THROW_ERROR("failed to remove the created dir");
    }
    return 0;
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_write_read),
    TEST_CASE(test_write_fdatasync_read),
    TEST_CASE(test_write_fsync_read),
    TEST_CASE(test_rename),
    TEST_CASE(test_readdir),
    TEST_CASE(test_truncate),
    TEST_CASE(test_mkdir_then_rmdir),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
