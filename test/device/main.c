#define _GNU_SOURCE
#include <sys/stat.h>
#include <sys/types.h>
#include <fcntl.h>
#include <poll.h>
#include <unistd.h>
#include <stdio.h>
#include <stdlib.h>
#include "test_fs.h"

// ============================================================================
// Test utilities
// ============================================================================

static int check_file_readable(const char *filename) {
    int fd;
    char buf[512] = {0};
    int len;
    if ((fd = open(filename, O_RDONLY)) < 0) {
        THROW_ERROR("failed to open the file");
    }
    if ((len = read(fd, buf, sizeof(buf))) != sizeof(buf)) {
        THROW_ERROR("failed to read the file");
    }
    close(fd);
    return 0;
}

static int check_file_writable(const char *filename) {
    int fd;
    char buf[512] = {0};
    int len;
    if ((fd = open(filename, O_WRONLY)) < 0) {
        THROW_ERROR("failed to open the file");
    }
    if ((len = write(fd, buf, sizeof(buf))) != sizeof(buf)) {
        THROW_ERROR("failed to read the file");
    }
    close(fd);
    return 0;
}

// ============================================================================
// Test cases for /dev/random, /dev/urandom, /dev/
// ============================================================================

int test_dev_null() {
    if (check_file_writable("/dev/null")) {
        THROW_ERROR("failed to write to /dev/null");
    }
    return 0;
}

int test_dev_zero() {
    if (check_file_readable("/dev/zero")) {
        THROW_ERROR("failed to read from /dev/null");
    }
    return 0;
}

int test_dev_random() {
    if (check_file_readable("/dev/random")) {
        THROW_ERROR("failed to read from /dev/random");
    }
    return 0;
}

int test_dev_urandom() {
    if (check_file_readable("/dev/urandom")) {
        THROW_ERROR("failed to read from /dev/urandom");
    }
    return 0;
}

int test_dev_urandom_fstat() {
    int fd;
    struct stat stat_buf;

    if ((fd = open("/dev/urandom", O_RDONLY)) < 0) {
        THROW_ERROR("failed to open the file");
    }
    if (fstat(fd, &stat_buf) < 0) {
        close(fd);
        THROW_ERROR("failed to fstat the file");
    }
    close(fd);
    if ((stat_buf.st_mode & S_IFMT) != S_IFCHR) {
        THROW_ERROR("not a character device");
    }
    return 0;
}

int test_dev_urandom_poll() {
    int fd;
    struct pollfd fds;

    if ((fd = open("/dev/urandom", O_RDONLY)) < 0) {
        THROW_ERROR("failed to open the file");
    }
    fds.fd = fd;
    fds.events = POLLIN;
    if (poll(&fds, 1, 5) <= 0) {
        close(fd);
        THROW_ERROR("failed to poll or file is not ready");
    }
    close(fd);
    if (fds.revents != POLLIN) {
        THROW_ERROR("not expected returned events");
    }
    return 0;
}

int test_dev_arandom() {
    if (check_file_readable("/dev/arandom")) {
        THROW_ERROR("failed to read from /dev/arandom");
    }
    return 0;
}

int test_dev_shm() {
    struct stat stat_buf;
    if (stat("/dev/shm", &stat_buf) < 0) {
        THROW_ERROR("failed to stat /dev/shm");
    }
    if (!S_ISDIR(stat_buf.st_mode)) {
        THROW_ERROR("failed to check if it is dir");
    }

    char *write_str = "Hello World\n";
    char *file_path = "/dev/shm/test_read_write.txt";
    int fd = open(file_path, O_WRONLY | O_CREAT | O_TRUNC, 00666);
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
    if (unlink(file_path) < 0) {
        THROW_ERROR("failed to unlink the file");
    }
    return 0;
}

int test_dev_fd() {
    char *file_path = "/root/hello_world";
    char *greetings = "hello";
    int fd = open(file_path, O_RDWR | O_CREAT | O_TRUNC, 00666);
    if (fd < 0) {
        THROW_ERROR("failed to open a file to write");
    }

    // Generate dev_fd path
    char dev_fd_path[20] = "/dev/fd/";
    char *fd_str = calloc(1, 4); // 4 bytes would be enough here.
    if (fd_str == NULL) {
        THROW_ERROR("calloc failed");
    }
    if (asprintf(&fd_str, "%d", fd) < 0) {
        THROW_ERROR("failed to asprintf");
    }
    strcat(dev_fd_path, fd_str);
    int dev_fd = open(dev_fd_path, O_WRONLY, 0666);
    if (dev_fd < 0) {
        THROW_ERROR("failed to open %s", dev_fd_path);
    }

    int len = write(dev_fd, greetings, strlen(greetings));
    if (len < 0) {
        THROW_ERROR("failed to write to %s", dev_fd_path);
    }

    char buf[10] = {0};
    len = read(fd, buf, len);
    if (len < 0) {
        THROW_ERROR("failed to read from %s", file_path);
    }

    if (strcmp(buf, greetings)) {
        THROW_ERROR("file content is wrong");
    }

    free(fd_str);
    close(dev_fd);
    close(fd);
    return 0;
}
// ============================================================================
// Test suite
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_dev_null),
    TEST_CASE(test_dev_zero),
    TEST_CASE(test_dev_random),
    TEST_CASE(test_dev_urandom),
    TEST_CASE(test_dev_urandom_fstat),
    TEST_CASE(test_dev_urandom_poll),
    TEST_CASE(test_dev_arandom),
    TEST_CASE(test_dev_shm),
    TEST_CASE(test_dev_fd),
};

int main() {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
