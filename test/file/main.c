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
// Test cases for file
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

static int __test_posix_fallocate(const char *file_path) {
    int fd = open(file_path, O_RDWR);
    if (fd < 0) {
        THROW_ERROR("failed to open a file to read/write");
    }
    off_t offset = -1;
    off_t len = 100;
    if (posix_fallocate(fd, offset, len) != EINVAL) {
        THROW_ERROR("failed to call posix_fallocate with invalid offset");
    }
    offset = 16;
    len = 0;
    if (posix_fallocate(fd, offset, len) != EINVAL) {
        THROW_ERROR("failed to call posix_fallocate with invalid len");
    }
    len = 48;
    if (posix_fallocate(fd, offset, len) != 0) {
        THROW_ERROR("failed to call posix_fallocate");
    }
    struct stat stat_buf;
    if (fstat(fd, &stat_buf) < 0) {
        THROW_ERROR("failed to stat file");
    }
    if (stat_buf.st_size < offset + len) {
        THROW_ERROR("failed to check the len after posix_fallocate");
    }

    char *read_buf = malloc(stat_buf.st_size);
    if (read_buf == NULL) {
        THROW_ERROR("failed to malloc buf to read");
    }
    if (read(fd, read_buf, stat_buf.st_size) != stat_buf.st_size) {
        THROW_ERROR("failed to read correct size of fallocated file");
    }

    free(read_buf);
    close(fd);
    return 0;
}

typedef int(*test_file_func_t)(const char *);

static int test_file_framework(test_file_func_t fn) {
    const char *file_path = "/root/test_filesystem_file_read_write.txt";

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

static int test_posix_fallocate() {
    return test_file_framework(__test_posix_fallocate);
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_write_read),
    TEST_CASE(test_pwrite_pread),
    TEST_CASE(test_writev_readv),
    TEST_CASE(test_lseek),
    TEST_CASE(test_posix_fallocate),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
