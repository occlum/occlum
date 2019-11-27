#include <sys/types.h>
#include <sys/ioctl.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <stdio.h>
#include <termios.h>
#include <unistd.h>
#include "test.h"

// ============================================================================
// Test cases for TTY ioctl
// ============================================================================

int test_tty_ioctl_TIOCGWINSZ(void) {
    struct winsize winsize;
    if (ioctl(STDOUT_FILENO, TIOCGWINSZ, &winsize) < 0) {
        THROW_ERROR("failed to ioctl TIOCGWINSZ");
    }
    return 0;
}

// ============================================================================
// Test cases for SGX ioctl
// ============================================================================

#define SGXIOC_IS_EDDM_SUPPORTED _IOR('s', 0, int)

int test_sgx_ioctl_SGXIOC_IS_EDDM_SUPPORTED(void) {
    int sgx_fd;
    if ((sgx_fd = open("/dev/sgx", O_RDONLY)) < 0) {
        THROW_ERROR("failed to open /dev/sgx ");
    }

    int is_edmm_supported = 0;
    if (ioctl(sgx_fd, SGXIOC_IS_EDDM_SUPPORTED, &is_edmm_supported) < 0) {
        THROW_ERROR("failed to ioctl /dev/sgx");
    }
    if (is_edmm_supported != 0) {
        THROW_ERROR("SGX EDMM supported are not expected to be enabled");
    }

    close(sgx_fd);
    return 0;
}

// ============================================================================
// Test suite
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_tty_ioctl_TIOCGWINSZ),
    TEST_CASE(test_sgx_ioctl_SGXIOC_IS_EDDM_SUPPORTED)
};

int main() {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
