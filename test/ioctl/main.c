#include <net/if.h>
#include <sys/socket.h>
#include <sys/types.h>
#include <sys/ioctl.h>
#include <sys/stat.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <termios.h>
#include <unistd.h>
#include <sgx_report.h>
#include <sgx_quote.h>
#include "test.h"

// ============================================================================
// Test cases for TTY ioctls
// ============================================================================

int test_tty_ioctl_TIOCGWINSZ(void) {
    struct winsize winsize;
    if (isatty(STDOUT_FILENO)) {
        if (ioctl(STDOUT_FILENO, TIOCGWINSZ, &winsize) < 0) {
            THROW_ERROR("failed to ioctl TIOCGWINSZ");
        }
    } else {
        // FIXME: /dev/tty should be opened. But it has not been implemented in Occlum yet.
        // So we just skip this test if STDOUT is redirected.
        printf("Warning: test_tty_ioctl_TIOCGWINSZ is skipped\n");
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
    const uint8_t              *sigrl_ptr;          // input (optional)
    uint32_t                    sigrl_len;          // input (optional)
    uint32_t                    quote_buf_len;      // input
    union {
        uint8_t                *as_buf;
        sgx_quote_t            *as_quote;
    } quote;                                        // output
} sgxioc_gen_quote_arg_t;

typedef struct {
    const sgx_target_info_t    *target_info;        // input (optinal)
    const sgx_report_data_t    *report_data;        // input (optional)
    sgx_report_t               *report;             // output
} sgxioc_create_report_arg_t;

#define SGXIOC_IS_EDMM_SUPPORTED _IOR('s', 0, int)
#define SGXIOC_GET_EPID_GROUP_ID _IOR('s', 1, sgx_epid_group_id_t)
#define SGXIOC_GEN_QUOTE         _IOWR('s', 2, sgxioc_gen_quote_arg_t)
#define SGXIOC_SELF_TARGET       _IOR('s', 3, sgx_target_info_t)
#define SGXIOC_CREATE_REPORT     _IOWR('s', 4, sgxioc_create_report_arg_t)
#define SGXIOC_VERIFY_REPORT     _IOW('s', 5, sgx_report_t)

// The max number of retries if ioctl returns EBUSY
#define IOCTL_MAX_RETRIES       20

typedef int(*sgx_ioctl_test_body_t)(int sgx_fd);

static int do_SGXIOC_IS_EDMM_SUPPORTED(int sgx_fd) {
    int is_edmm_supported = 0;
    if (ioctl(sgx_fd, SGXIOC_IS_EDMM_SUPPORTED, &is_edmm_supported) < 0) {
        THROW_ERROR("failed to ioctl /dev/sgx");
    }
    if (is_edmm_supported != 0) {
        THROW_ERROR("SGX EDMM supported are not expected to be enabled");
    }
    return 0;
}

static int do_SGXIOC_GET_EPID_GROUP_ID(int sgx_fd) {
    int nretries = 0;
    while (nretries < IOCTL_MAX_RETRIES) {
        sgx_epid_group_id_t epid_group_id = { 0 };
        int ret = ioctl(sgx_fd, SGXIOC_GET_EPID_GROUP_ID, &epid_group_id);
        if (ret == 0) {
            break;
        } else if (errno != EBUSY) {
            THROW_ERROR("failed to ioctl /dev/sgx");
        }

        printf("WARN: /dev/sgx is temporarily busy. Try again after 1 second.");
        sleep(1);
        nretries++;
    }
    if (nretries == IOCTL_MAX_RETRIES) {
        THROW_ERROR("failed to ioctl /dev/sgx due to timeout");
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
        .quote = { .as_buf = (uint8_t *) quote_buf }    // output
    };
    int nretries = 0;
    while (nretries < IOCTL_MAX_RETRIES) {
        int ret = ioctl(sgx_fd, SGXIOC_GEN_QUOTE, &gen_quote_arg);
        if (ret == 0) {
            break;
        } else if (errno != EBUSY) {
            THROW_ERROR("failed to ioctl /dev/sgx");
        }

        printf("WARN: /dev/sgx is temporarily busy. Try again after 1 second.");
        sleep(1);
        nretries++;
    }
    if (nretries == IOCTL_MAX_RETRIES) {
        THROW_ERROR("failed to ioctl /dev/sgx due to timeout");
    }

    sgx_quote_t *quote = (sgx_quote_t *)quote_buf;
    if (quote->sign_type != SGX_LINKABLE_SIGNATURE) {
        THROW_ERROR("invalid quote: wrong sign type");
    }
    if (quote->signature_len == 0) {
        THROW_ERROR("invalid quote: zero-length signature");
    }
    if (memcmp(&gen_quote_arg.report_data, &quote->report_body.report_data,
               sizeof(sgx_report_data_t)) != 0) {
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

static int do_SGXIOC_SELF_TARGET(int sgx_fd) {
    sgx_target_info_t target_info;
    if (ioctl(sgx_fd, SGXIOC_SELF_TARGET, &target_info) < 0) {
        THROW_ERROR("failed to ioctl /dev/sgx");
    }
    return 0;
}

static int do_SGXIOC_CREATE_AND_VERIFY_REPORT(int sgx_fd) {
    sgx_target_info_t target_info;
    if (ioctl(sgx_fd, SGXIOC_SELF_TARGET, &target_info) < 0) {
        THROW_ERROR("failed to ioctl /dev/sgx");
    }
    sgx_report_data_t report_data;
    sgx_report_t report;

    sgxioc_create_report_arg_t args[] = {
        {
            .target_info = (const sgx_target_info_t *) &target_info,
            .report_data = NULL,
            .report = &report
        },
        {
            .target_info = (const sgx_target_info_t *) &target_info,
            .report_data = (const sgx_report_data_t *) &report_data,
            .report = &report
        }
    };
    for (int arg_i = 0; arg_i < ARRAY_SIZE(args); arg_i++) {
        memset(&report, 0, sizeof(report));
        sgxioc_create_report_arg_t *arg = &args[arg_i];
        if (ioctl(sgx_fd, SGXIOC_CREATE_REPORT, arg) < 0) {
            THROW_ERROR("failed to create report");
        }
        if (ioctl(sgx_fd, SGXIOC_VERIFY_REPORT, &report) < 0) {
            THROW_ERROR("failed to verify report");
        }
    }
    return 0;
}

int test_sgx_ioctl_SGXIOC_IS_EDMM_SUPPORTED(void) {
    return do_sgx_ioctl_test(do_SGXIOC_IS_EDMM_SUPPORTED);
}

int test_sgx_ioctl_SGXIOC_GET_EPID_GROUP_ID(void) {
    return do_sgx_ioctl_test(do_SGXIOC_GET_EPID_GROUP_ID);
}

int test_sgx_ioctl_SGXIOC_GEN_QUOTE(void) {
    return do_sgx_ioctl_test(do_SGXIOC_GEN_QUOTE);
}

int test_sgx_ioctl_SGXIOC_SELF_TARGET(void) {
    return do_sgx_ioctl_test(do_SGXIOC_SELF_TARGET);
}

int test_sgx_ioctl_SGXIOC_CREATE_AND_VERIFY_REPORT(void) {
    return do_sgx_ioctl_test(do_SGXIOC_CREATE_AND_VERIFY_REPORT);
}

#define CONFIG_SIZE  512
int test_ioctl_SIOCGIFCONF(void) {
    struct ifreq *req;
    struct ifconf conf;
    char *buf = (char *)malloc(CONFIG_SIZE);
    if (buf == NULL) {
        THROW_ERROR("malloc failed");
    }
    memset(buf, 0, CONFIG_SIZE);

    int sock = socket(AF_INET, SOCK_STREAM, 0);

    conf.ifc_len = 0;
    conf.ifc_buf = buf;
    if (ioctl(sock, SIOCGIFCONF, &conf) < 0) {
        close(sock);
        THROW_ERROR("empty length ioctl failed");
    }

    if (conf.ifc_len != 0) {
        close(sock);
        THROW_ERROR("wrong returned length");
    }

    conf.ifc_len = CONFIG_SIZE;
    conf.ifc_buf = 0;
    if (ioctl(sock, SIOCGIFCONF, &conf) < 0) {
        close(sock);
        THROW_ERROR("empty buffer ioctl failed");
    }

    int ret_len = conf.ifc_len;

    // use a larger buffer when the original one is insufficient
    if (ret_len > CONFIG_SIZE) {
        free(buf);

        char *new_buf = (char *)malloc(ret_len);
        if (new_buf == NULL) {
            close(sock);
            THROW_ERROR("malloc failed");
        }
        buf = new_buf;
        memset(buf, 0, ret_len);
    } else {
        conf.ifc_len = CONFIG_SIZE;
    }

    conf.ifc_buf = buf;
    if (ioctl(sock, SIOCGIFCONF, &conf) < 0) {
        close(sock);
        THROW_ERROR("buffer passed ioctl failed");
    }

    if (conf.ifc_len != ret_len) {
        close(sock);
        THROW_ERROR("wrong return length");
    }

    close(sock);

    req = (struct ifreq *)buf;
    int num = conf.ifc_len / sizeof (struct ifreq);

    printf("    interface names got:\n");
    for (int i = 0; i < num; i++) {
        printf("    %d: %s\n", i + 1, req->ifr_name);
        req ++;
    }

    return 0;
}

int test_ioctl_FIONBIO(void) {
    int sock = socket(AF_INET, SOCK_STREAM, 0);

    int on = 1;
    if (ioctl(sock, FIONBIO, &on) < 0) {
        close(sock);
        THROW_ERROR("ioctl FIONBIO failed");
    }

    int actual_flags = fcntl(sock, F_GETFL);
    if ((actual_flags & O_NONBLOCK) == 0) {
        close(sock);
        THROW_ERROR("failed to check the O_NONBLOCK flag after FIONBIO");
    }

    close(sock);
    return 0;
}

// ============================================================================
// Test suite
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_tty_ioctl_TIOCGWINSZ),
    TEST_CASE(test_sgx_ioctl_SGXIOC_IS_EDMM_SUPPORTED),
    TEST_CASE(test_sgx_ioctl_SGXIOC_GET_EPID_GROUP_ID),
    TEST_CASE(test_sgx_ioctl_SGXIOC_GEN_QUOTE),
    TEST_CASE(test_sgx_ioctl_SGXIOC_SELF_TARGET),
    TEST_CASE(test_sgx_ioctl_SGXIOC_CREATE_AND_VERIFY_REPORT),
    TEST_CASE(test_ioctl_SIOCGIFCONF),
    TEST_CASE(test_ioctl_FIONBIO),
};

int main() {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
