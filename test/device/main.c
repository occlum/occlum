#include <fcntl.h>
#include <unistd.h>
#include <stdio.h>
#include "test.h"

// ============================================================================
// Test utilities
// ============================================================================

static int check_file_readable(const char* filename) {
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

static int check_file_writable(const char* filename) {
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

int test_dev_arandom() {
    if (check_file_readable("/dev/arandom")) {
        THROW_ERROR("failed to read from /dev/arandom");
    }
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
    TEST_CASE(test_dev_arandom),
};

int main() {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
