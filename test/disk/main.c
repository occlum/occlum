#define _GNU_SOURCE
#include <fcntl.h>
#include "test.h"

#define KB              (1024UL)
#define MB              (1024UL * 1024UL)

#define BLOCK_SIZE      (4 * KB)
#define TOTAL_BYTES     (4 * MB)

// ============================================================================
// Helper function
// ============================================================================

static int create_disk(const char *disk_path) {
    int fd;
    int flags = O_RDWR | O_CREAT;
    int mode = 00666;
    fd = open(disk_path, flags, mode);
    if (fd < 0) {
        THROW_ERROR("failed to create a disk");
    }
    return fd;
}

static int open_disk(const char *disk_path) {
    int fd;
    int flags = O_RDWR;
    int mode = 00666;
    fd = open(disk_path, flags, mode);
    if (fd < 0) {
        THROW_ERROR("failed to open a disk");
    }
    return fd;
}

// ============================================================================
// Test cases for disk
// ============================================================================

static int __test_write(int fd) {
    char wbuf[BLOCK_SIZE];

    for (size_t i = 0; i < TOTAL_BYTES; i += BLOCK_SIZE) {
        memset(wbuf, i, BLOCK_SIZE);
        off_t offset = i;
        int written_bytes = pwrite(fd, wbuf, BLOCK_SIZE, offset);
        if (written_bytes != BLOCK_SIZE) {
            THROW_ERROR("disk write failed");
        }
    }

    close(fd);
    return 0;
}

static int __test_open(const char *disk_path) {
    return open_disk(disk_path);
}

static int __test_read(int fd) {
    char rbuf[BLOCK_SIZE] = { 0 };

    for (size_t i = 0; i < TOTAL_BYTES; i += BLOCK_SIZE) {
        off_t offset = i;
        int read_nbytes = pread(fd, rbuf, BLOCK_SIZE, offset);
        if (read_nbytes != BLOCK_SIZE) {
            THROW_ERROR("disk read failed");
        }

        int expected_byte_val = i;
        if (check_bytes_in_buf(rbuf, BLOCK_SIZE, expected_byte_val) < 0) {
            THROW_ERROR("Incorrect data");
        }
    }

    close(fd);
    return 0;
}

static int test_disk_framework(const char *disk_type) {
    const char *prefix = "/dev/";
    char disk_path[strlen(prefix) + strlen(disk_type) + 1];
    strcpy(disk_path, prefix);
    strcat(disk_path, disk_type);
    int fd;

    fd = create_disk(disk_path);
    __test_write(fd);
    fd = __test_open(disk_path);
    __test_read(fd);
    return 0;
}

static int test_jindisk() {
    return test_disk_framework("jindisk");
}

static int test_pfs_disk() {
    return test_disk_framework("pfs_disk");
}

static int test_crypt_sync_disk() {
    return test_disk_framework("crypt_sync_disk");
}

static int test_crypt_iou_disk() {
    return test_disk_framework("crypt_iou_disk");
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_jindisk),
    TEST_CASE(test_pfs_disk),
    TEST_CASE(test_crypt_sync_disk),
    TEST_CASE(test_crypt_iou_disk),
};

int main(int argc, const char *argv[]) {
    if (test_suite_run(test_cases, ARRAY_SIZE(test_cases)) < 0) {
        return -1;
    }
    sync();
    return 0;
}
