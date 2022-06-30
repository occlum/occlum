#define _GNU_SOURCE
#include <sys/stat.h>
#include <sys/uio.h>
#include <errno.h>
#include <fcntl.h>
#include <stdlib.h>
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
// Test cases for fs
// ============================================================================

static int __test_write_read(const char *file_path) {
    char *write_str = "Hello World\n";
    int fd;

    fd = open(file_path, O_WRONLY);
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

static int __test_pwrite_pread(const char *file_path) {
    char *write_str = "Hello World\n";
    char read_buf[128] = { 0 };
    int ret, fd;

    fd = open(file_path, O_WRONLY);
    if (fd < 0) {
        THROW_ERROR("failed to open a file to pwrite");
    }
    if (pwrite(fd, write_str, strlen(write_str), 1) <= 0) {
        THROW_ERROR("failed to pwrite");
    }
    ret = pwrite(fd, write_str, strlen(write_str), -1);
    if (ret >= 0 || errno != EINVAL) {
        THROW_ERROR("check pwrite with negative offset fail");
    }
    close(fd);
    fd = open(file_path, O_RDONLY);
    if (fd < 0) {
        THROW_ERROR("failed to open a file to pread");
    }
    if (pread(fd, read_buf, sizeof(read_buf), 1) != strlen(write_str)) {
        THROW_ERROR("failed to pread");
    }
    if (strcmp(write_str, read_buf) != 0) {
        THROW_ERROR("the message read from the file is not as it was written");
    }
    ret = pread(fd, write_str, strlen(write_str), -1);
    if (ret >= 0 || errno != EINVAL) {
        THROW_ERROR("check pread with negative offset fail");
    }
    close(fd);
    return 0;
}

static int __test_writev_readv(const char *file_path) {
    const char *iov_msg[2] = {"hello_", "world!"};
    char read_buf[128] = { 0 };
    struct iovec iov[2];
    int fd, len = 0;

    fd = open(file_path, O_WRONLY);
    if (fd < 0) {
        THROW_ERROR("failed to open a file to writev");
    }
    for (int i = 0; i < 2; ++i) {
        iov[i].iov_base = (void *)iov_msg[i];
        iov[i].iov_len = strlen(iov_msg[i]);
        len += iov[i].iov_len;
    }
    if (writev(fd, iov, 2) != len) {
        THROW_ERROR("failed to write vectors to the file");
        return -1;
    }
    close(fd);
    fd = open(file_path, O_RDONLY);
    if (fd < 0) {
        THROW_ERROR("failed to open a file to readv");
    }
    iov[0].iov_base = read_buf;
    iov[0].iov_len = strlen(iov_msg[0]);
    iov[1].iov_base = read_buf + strlen(iov_msg[0]);
    iov[1].iov_len = strlen(iov_msg[1]);
    if (readv(fd, iov, 2) != len) {
        THROW_ERROR("failed to read vectors from the file");
    }
    if (memcmp(read_buf, iov_msg[0], strlen(iov_msg[0])) != 0 ||
            memcmp(read_buf + strlen(iov_msg[0]), iov_msg[1], strlen(iov_msg[1])) != 0) {
        THROW_ERROR("the message read from the file is not as it was written");
    }
    close(fd);
    return 0;
}

static int __test_lseek(const char *file_path) {
    char *write_str = "Hello World\n";
    char read_buf[128] = { 0 };
    int fd, offset, ret;

    fd = open(file_path, O_RDWR);
    if (fd < 0) {
        THROW_ERROR("failed to open a file to read/write");
    }
    if (write(fd, write_str, strlen(write_str)) <= 0) {
        THROW_ERROR("failed to write");
    }
    /* make sure offset is in range (0, strlen(write_str)) */
    offset = 2;
    if (lseek(fd, offset, SEEK_SET) != offset) {
        THROW_ERROR("failed to lseek the file");
    }
    if (read(fd, read_buf, sizeof(read_buf)) >= strlen(write_str)) {
        THROW_ERROR("failed to read from offset");
    }
    if (strcmp(write_str + offset, read_buf) != 0) {
        THROW_ERROR("the message read from the offset is wrong");
    }
    offset = -1;
    ret = lseek(fd, offset, SEEK_SET);
    if (ret >= 0 || errno != EINVAL) {
        THROW_ERROR("check lseek with negative offset fail");
    }
    if (lseek(fd, 0, SEEK_END) != strlen(write_str)) {
        THROW_ERROR("faild to lseek to the end of the file");
    }
    close(fd);
    return 0;
}

static int __test_rename(const char *file_path) {
    char *rename_path = "/async_sfs/test_async_sfs_rename.txt";
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
    char dir_buf[PATH_MAX] = { 0 };
    char *dir_name;
    bool found = false;

    if (fs_split_path(file_path, dir_buf, &dir_name, base_buf, &base_name) < 0) {
        THROW_ERROR("failed to split path");
    }
    dirp = opendir(dir_name);
    if (dirp == NULL) {
        THROW_ERROR("failed to open directory: %s", dir_name);
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


typedef int(*test_file_func_t)(const char *);

static int test_file_framework(test_file_func_t fn) {
    const char *file_path = "/async_sfs/test_async_fs_file.txt";

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
    return test_file_framework(__test_write_read);
}

static int test_pwrite_pread() {
    return test_file_framework(__test_pwrite_pread);
}

static int test_writev_readv() {
    return test_file_framework(__test_writev_readv);
}

static int test_lseek() {
    return test_file_framework(__test_lseek);
}

static int test_rename() {
    return test_file_framework(__test_rename);
}

static int test_readdir() {
    return test_file_framework(__test_readdir);
}

static int test_mkdir_and_rmdir() {
    struct stat stat_buf;
    mode_t mode = 00775;
    const char *dir_path = "/async_sfs/test_async_fs_dir";

    if (mkdir(dir_path, mode) < 0) {
        THROW_ERROR("failed to mkdir");
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

    int ret = stat(dir_path, &stat_buf);
    if (!(ret < 0 && errno == ENOENT)) {
        THROW_ERROR("stat on \"%s\" should return ENOENT", dir_path);
    }
    return 0;
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_write_read),
    TEST_CASE(test_pwrite_pread),
    TEST_CASE(test_writev_readv),
    TEST_CASE(test_lseek),
    TEST_CASE(test_rename),
    TEST_CASE(test_readdir),
    TEST_CASE(test_mkdir_and_rmdir),
};

int main(int argc, const char *argv[]) {
    if (test_suite_run(test_cases, ARRAY_SIZE(test_cases)) < 0) {
        return -1;
    }
    sync();
    return 0;
}
