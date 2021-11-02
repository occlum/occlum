#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <unistd.h>
#include <string.h>
#include <stdio.h>
#include "test_fs.h"

// ============================================================================
// Helper function
// ============================================================================

static int create_file(const char *file_path) {
    int fd;
    int flags = O_WRONLY | O_CREAT | O_TRUNC;
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
// Test cases for truncate
// ============================================================================

static int __test_truncate(const char *file_path) {
    int fd;
    off_t len = 128;

    fd = open(file_path, O_WRONLY);
    if (fd < 0) {
        THROW_ERROR("failed to open a file to truncate");
    }
    if (ftruncate(fd, len) < 0) {
        THROW_ERROR("failed to call ftruncate");
    }
    struct stat stat_buf;
    if (fstat(fd, &stat_buf) < 0) {
        THROW_ERROR("failed to stat file");
    }
    if (stat_buf.st_size != len) {
        THROW_ERROR("failed to check the len after ftruncate");
    }
    close(fd);

    len = 256;
    if (truncate(file_path, len) < 0) {
        THROW_ERROR("failed to call truncate");
    }
    if (stat(file_path, &stat_buf) < 0) {
        THROW_ERROR("failed to stat file");
    }
    if (stat_buf.st_size != len) {
        THROW_ERROR("failed to check the len after truncate");
    }

    return 0;
}

static int __test_open_truncate_existing_file(const char *file_path) {
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

    fd = open(file_path, O_RDWR | O_TRUNC);
    if (fd < 0) {
        THROW_ERROR("failed to open an existing file with O_TRUNC");
    }
    struct stat stat_buf;
    if (fstat(fd, &stat_buf) < 0) {
        THROW_ERROR("failed to stat file");
    }
    if (stat_buf.st_size != 0) {
        THROW_ERROR("failed to check the len after open with O_TRUNC");
    }
    close(fd);

    return 0;
}

static int __test_truncate_then_read(const char *file_path) {
    size_t file_len = 32;
    off_t small_len = 16;
    off_t big_len = 48;
    char read_buf[128] = { 0 };
    int fd;

    fd = open(file_path, O_RDWR);
    if (fd < 0) {
        THROW_ERROR("failed to open file");
    }

    // truncate to small length, then read
    if (fill_file_with_repeated_bytes(fd, file_len, 0xfa) < 0) {
        THROW_ERROR("");
    }
    if (ftruncate(fd, small_len) < 0) {
        THROW_ERROR("failed to call ftruncate to small length");
    }
    if (lseek(fd, 0, SEEK_SET) < 0) {
        THROW_ERROR("failed to call lseek");
    }
    if (read(fd, read_buf, sizeof(read_buf)) != small_len) {
        THROW_ERROR("failed to check read with small length");
    }
    if (check_bytes_in_buf(read_buf, small_len, 0xfa) < 0) {
        THROW_ERROR("failed to check the read buf after truncate with smaller length");
    }

    // truncate to big length, then check the file content between small length and big length
    if (ftruncate(fd, big_len) < 0) {
        THROW_ERROR("failed to call ftruncate");
    }
    if (lseek(fd, small_len, SEEK_SET) < 0) {
        THROW_ERROR("failed to call lseek");
    }
    memset(read_buf, 0x00, sizeof(read_buf));
    if (read(fd, read_buf, sizeof(read_buf)) != big_len - small_len) {
        THROW_ERROR("failed to check read with big length");
    }
    if (check_bytes_in_buf(read_buf, big_len - small_len, 0x00) < 0) {
        THROW_ERROR("failed to check the read buf after truncate with bigger lenghth");
    }
    close(fd);
    return 0;
}

static int __test_truncate_then_write(const char *file_path) {
    size_t file_len = 32;
    off_t small_len = 16;
    char write_buf[16] = { 0 };
    char read_buf[16] = { 0 };
    int fd;

    fd = open(file_path, O_RDWR);
    if (fd < 0) {
        THROW_ERROR("failed to open file");
    }

    // truncate file to small length, then write beyond the length
    if (fill_file_with_repeated_bytes(fd, file_len, 0xfa) < 0) {
        THROW_ERROR("");
    }
    if (ftruncate(fd, small_len) < 0) {
        THROW_ERROR("failed to call ftruncate to small length");
    }
    if (lseek(fd, file_len, SEEK_SET) < 0) {
        THROW_ERROR("failed to call lseek");
    }
    memset(write_buf, 0xaa, sizeof(write_buf));
    if (write(fd, write_buf, sizeof(write_buf)) != sizeof(write_buf)) {
        THROW_ERROR("failed to write buffer");
    }

    // check the file content between small length and old length
    if (lseek(fd, small_len, SEEK_SET) < 0) {
        THROW_ERROR("failed to call lseek");
    }
    if (read(fd, read_buf, sizeof(read_buf)) != file_len - small_len) {
        THROW_ERROR("failed to read buf");
    }
    if (check_bytes_in_buf(read_buf, file_len - small_len, 0x00) < 0) {
        THROW_ERROR("failed to check the read buf after write beyond the length");
    }
    close(fd);
    return 0;
}

typedef int(*test_file_func_t)(const char *);

static int test_file_framework(test_file_func_t fn) {
    const char *file_path = "/root/test_filesystem_truncate.txt";

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

static int test_truncate() {
    return test_file_framework(__test_truncate);
}

static int test_open_truncate_existing_file() {
    return test_file_framework(__test_open_truncate_existing_file);
}

static int test_truncate_then_read() {
    return test_file_framework(__test_truncate_then_read);
}

static int test_truncate_then_write() {
    return test_file_framework(__test_truncate_then_write);
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_truncate),
    TEST_CASE(test_open_truncate_existing_file),
    TEST_CASE(test_truncate_then_write),
    TEST_CASE(test_truncate_then_read),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
