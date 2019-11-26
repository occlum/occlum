#include <sys/types.h>
#include <sys/ioctl.h>
#include <sys/stat.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <termios.h>
#include <unistd.h>
#include <sgx_quote.h>
#include "test.h"

// ============================================================================
// Test cases for TTY ioctls
// ============================================================================

int test_tty_ioctl_TIOCGWINSZ(void) {
    struct winsize winsize;
    if (ioctl(STDOUT_FILENO, TIOCGWINSZ, &winsize) < 0) {
        THROW_ERROR("failed to ioctl TIOCGWINSZ");
    }
    return 0;
}

// ============================================================================
// Test cases for SGX ioctls
// ============================================================================

typedef struct {
    sgx_report_data_t           report_data;        // input
    sgx_quote_sign_type_t       quote_type;         // input
    sgx_spid_t                  spid;               // input
    sgx_quote_nonce_t           nonce;              // input
    const uint8_t*              sigrl_ptr;          // input (optional)
    uint32_t                    sigrl_len;          // input (optional)
    uint32_t                    quote_buf_len;      // input
    union {
        uint8_t*                as_buf;
        sgx_quote_t*            as_quote;
    } quote;                                        // output
} sgxioc_gen_quote_arg_t;

#define SGXIOC_IS_EDDM_SUPPORTED _IOR('s', 0, int)
#define SGXIOC_GET_EPID_GROUP_ID _IOR('s', 1, sgx_epid_group_id_t)
#define SGXIOC_GEN_QUOTE         _IOWR('s', 2, sgxioc_gen_quote_arg_t)

typedef int(*sgx_ioctl_test_body_t)(int sgx_fd);

static int do_SGXIOC_IS_EDDM_SUPPORTED(int sgx_fd) {
    int is_edmm_supported = 0;
    if (ioctl(sgx_fd, SGXIOC_IS_EDDM_SUPPORTED, &is_edmm_supported) < 0) {
        THROW_ERROR("failed to ioctl /dev/sgx");
    }
    if (is_edmm_supported != 0) {
        THROW_ERROR("SGX EDMM supported are not expected to be enabled");
    }
    return 0;
}

static int do_SGXIOC_GET_EPID_GROUP_ID(int sgx_fd) {
    sgx_epid_group_id_t epid_group_id = { 0 };
    if (ioctl(sgx_fd, SGXIOC_GET_EPID_GROUP_ID, &epid_group_id) < 0) {
        THROW_ERROR("failed to ioctl /dev/sgx");
    }
    return 0;
}

static int do_SGXIOC_GEN_QUOTE(int sgx_fd) {
    uint8_t quote_buf[2048] = { 0 };
    sgxioc_gen_quote_arg_t gen_quote_arg = {
        .report_data = { { 0 } },                       // input (empty is ok)
        .quote_type = SGX_LINKABLE_SIGNATURE,           // input
        .spid = { { 0 } },                              // input (empty is ok)
        .nonce = { { 0 } },                             // input (empty is ok)
        .sigrl_ptr = NULL,                              // input (optional)
        .sigrl_len = 0,                                 // input (optional)
        .quote_buf_len = sizeof(quote_buf),             // input
        .quote = { .as_buf = (uint8_t*) quote_buf }     // output
    };

    while (1) {
        int ret = ioctl(sgx_fd, SGXIOC_GEN_QUOTE, &gen_quote_arg);
        if (ret == 0) {
            break;
        }
        else if (errno == EAGAIN) {
            printf("WARN: /dev/sgx is temporarily busy. Try again after 1 second.");
            sleep(1);
        }
        else {
            THROW_ERROR("failed to ioctl /dev/sgx");
        }
    }

    sgx_quote_t* quote = (sgx_quote_t*)quote_buf;
    if (quote->sign_type != SGX_LINKABLE_SIGNATURE) {
        THROW_ERROR("invalid quote: wrong sign type");
    }
    if (quote->signature_len == 0) {
        THROW_ERROR("invalid quote: zero-length signature");
    }
    if (memcmp(&gen_quote_arg.report_data, &quote->report_body.report_data, sizeof(sgx_report_data_t)) != 0) {
        THROW_ERROR("invalid quote: wrong report data");
    }
    return 0;
}

static int do_sgx_ioctl_test(sgx_ioctl_test_body_t test_body) {
    // Init test
    int sgx_fd;
    if ((sgx_fd = open("/dev/sgx", O_RDONLY)) < 0) {
        THROW_ERROR("failed to open /dev/sgx ");
    }

    // Do test
    int ret = test_body(sgx_fd);

    // Clean up test
    close(sgx_fd);
    return ret;
}

int test_sgx_ioctl_SGXIOC_IS_EDDM_SUPPORTED(void) {
    return do_sgx_ioctl_test(do_SGXIOC_IS_EDDM_SUPPORTED);
}

int test_sgx_ioctl_SGXIOC_GET_EPID_GROUP_ID(void) {
    return do_sgx_ioctl_test(do_SGXIOC_GET_EPID_GROUP_ID);
}

int test_sgx_ioctl_SGXIOC_GEN_QUOTE(void) {
    return do_sgx_ioctl_test(do_SGXIOC_GEN_QUOTE);
}

// ============================================================================
// Test suite
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_tty_ioctl_TIOCGWINSZ),
    TEST_CASE(test_sgx_ioctl_SGXIOC_IS_EDDM_SUPPORTED),
    TEST_CASE(test_sgx_ioctl_SGXIOC_GET_EPID_GROUP_ID),
    TEST_CASE(test_sgx_ioctl_SGXIOC_GEN_QUOTE)
};

int main() {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
