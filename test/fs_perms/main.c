#include <sys/types.h>
#include <sys/uio.h>
#include <fcntl.h>
#include "test_fs.h"

// ============================================================================
// Helper macros
// ============================================================================

//#define PRINT_DBG(msg) printf("%s %s %d ", msg, __FUNCTION__, __LINE__);
#define PRINT_DBG(msg)
#define OK (0)
#define NG (-1)

#define NUM_TEST_CASES   6
#define NUM_TEST_FILES   5

// ============================================================================
// Helper functions
// ============================================================================

static int open_file(const char *filename, int flags, int mode) {
    int fd = -1;
    if ((fd = open(filename, flags, mode)) < 0) {
        PRINT_DBG("ERROR: failed to open a file\n");
    }
    return fd;
}

static const char *write_msg = "Hello SEFS 1234567890\n";

static int write_file(int fd) {
    int len = strlen(write_msg);
    if ((len = write(fd, write_msg, len) <= 0)) {
        PRINT_DBG("ERROR: failed to write to the file\n");
        return -1;
    }
    fsync(fd);
    close(fd);
    return 0;
}

static int read_file(int fd) {
    int len;
    char read_buf[128] = {0};
    if ((len = read(fd, read_buf, sizeof(read_buf) - 1)) <= 0) {
        PRINT_DBG("ERROR: failed to read from the file\n");
        return -1;
    }
    close(fd);

    if (strcmp(write_msg, read_buf) != 0) {
        PRINT_DBG("ERROR: the message read from the file is not as it was written\n");
        return -1;
    }
    return 0;
}


// for each file in test_filename
//      open the file with the given flags
//      do read or write according to do_write
//      check the result of the read/write with the given expected_result
static int do_perm_tests(
    const char **files,
    size_t num_files,
    int flags, int do_write,
    int *expected_results) {
    flags |= O_CREAT;
    for (size_t i = 0; i < num_files; i++) {
        const char *filename = files[i];
        int expected_result = expected_results[i];

        int fd = open_file(filename, flags, 0666);
        if (fd < 0 && fd != expected_result) {
            return -1;
        }
        int result = do_write ? write_file(fd) : read_file(fd);
        if (result != expected_result) {
            return -1;
        }
    }
    return 0;
}

// ============================================================================
// Test cases
// ============================================================================

// Test files
static const char *test_files[NUM_TEST_FILES] = {
    "/test_fs_perms.txt",
    "/bin/test_fs_perms.txt",
    "/lib/test_fs_perms.txt",
    "/root/test_fs_perms.txt",
    "/host/test_fs_perms.txt",
};

// Test cases X Test files -> Test Results
static int test_expected_results[NUM_TEST_CASES][NUM_TEST_FILES] = {
    // test_open_ro_then_write()
    {NG, NG, NG, NG, NG},
    // test_open_wo_then_write()
    {OK, OK, OK, OK, OK},
    // test_open_rw_then_write()
    {OK, OK, OK, OK, OK},
    // test_open_ro_then_read()
    {OK, OK, OK, OK, OK},
    // test_open_wo_then_read()
    {NG, NG, NG, NG, NG},
    // test_open_rw_then_read()
    {OK, OK, OK, OK, OK},
};

int test_open_ro_then_write() {
    return do_perm_tests(test_files, NUM_TEST_FILES,
                         O_RDONLY, 1, test_expected_results[0]);
}

int test_open_wo_then_write() {
    return do_perm_tests(test_files, NUM_TEST_FILES,
                         O_WRONLY, 1, test_expected_results[1]);
}

int test_open_rw_then_write() {
    return do_perm_tests(test_files, NUM_TEST_FILES,
                         O_RDWR, 1, test_expected_results[2]);
}

int test_open_ro_then_read() {
    return do_perm_tests(test_files, NUM_TEST_FILES,
                         O_RDONLY, 0, test_expected_results[3]);
}

int test_open_wo_then_read() {
    return do_perm_tests(test_files, NUM_TEST_FILES,
                         O_WRONLY, 0, test_expected_results[4]);
}

int test_open_rw_then_read() {
    return do_perm_tests(test_files, NUM_TEST_FILES,
                         O_RDWR, 0, test_expected_results[5]);
}

// ============================================================================
// Test suite main
// ============================================================================

test_case_t test_cases[NUM_TEST_CASES] = {
    TEST_CASE(test_open_ro_then_write),
    TEST_CASE(test_open_wo_then_write),
    TEST_CASE(test_open_rw_then_write),
    TEST_CASE(test_open_ro_then_read),
    TEST_CASE(test_open_wo_then_read),
    TEST_CASE(test_open_rw_then_read)
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, NUM_TEST_CASES);
}
