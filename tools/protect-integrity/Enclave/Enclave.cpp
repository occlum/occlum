#include "Enclave_t.h"
#include <stdio.h>
#include <string.h>
#include <sgx_trts.h>
#include <sgx_tprotected_fs.h>

// ==========================================================================
//  Helper functions
// ==========================================================================

#define PRINTF_BUFSIZE          512

static int printf(const char *fmt, ...) {
    char buf[PRINTF_BUFSIZE] = {0};
    va_list args;
    va_start(args, fmt);
    vsnprintf(buf, PRINTF_BUFSIZE, fmt, args);
    va_end(args);
    ocall_print(buf);
    return 0;
}

static int eprintf(const char *fmt, ...) {
    char buf[PRINTF_BUFSIZE] = {0};
    va_list args;
    va_start(args, fmt);
    vsnprintf(buf, PRINTF_BUFSIZE, fmt, args);
    va_end(args);
    ocall_eprint(buf);
    return 0;
}

static void print_mac(sgx_aes_gcm_128bit_tag_t *mac) {
    unsigned char *bytes = (unsigned char *) mac;
    for (size_t bi = 0; bi < sizeof(*mac); bi++) {
        if (bi != 0) { printf("-"); }
        printf("%02x", bytes[bi] & 0xFF);
    }
    printf("\n");
}

static int open_for_write(const char *path) {
    int fd = 0;
    ocall_open_for_write(&fd, path);
    return fd;
}

static int open_for_read(const char *path) {
    int fd = 0;
    ocall_open_for_read(&fd, path);
    return fd;
}

static ssize_t read(int fd, void *buf, size_t size) {
    ssize_t ret = 0;
    ocall_read(&ret, fd, buf, size);
    return ret;
}

static ssize_t write(int fd, const void *buf, size_t size) {
    ssize_t ret = 0;
    ocall_write(&ret, fd, buf, size);
    return ret;
}

static int close(int fd) {
    int ret = 0;
    ocall_close(&ret, fd);
    return ret;
}

// ==========================================================================
//  ECalls
// ==========================================================================

int ecall_protect(const char *input_path, const char *output_path) {
    int input_file = -1;
    SGX_FILE *output_file = NULL;
    size_t len;
    char buf[4 * 1024];

    input_file = open_for_read(input_path);
    if (input_file < 0) {
        eprintf("Error: cannot open the input file at %s\n", input_path);
        goto on_error;
    }

    output_file = sgx_fopen_integrity_only(output_path, "w");
    if (output_file == NULL) {
        eprintf("Error: cannot create the output file %s\n", output_path);
        goto on_error;
    }

    while ((len = read(input_file, buf, sizeof(buf))) > 0) {
        if (sgx_fwrite(buf, 1, len, output_file) != len) {
            eprintf("Error: failed to write to the output file %s\n", output_path);
            goto on_error;
        }
    }

    close(input_file);
    sgx_fclose(output_file);
    return 0;
on_error:
    if (input_file >= 0) {
        close(input_file);
    }
    if (output_file != NULL) {
        sgx_fclose(output_file);
        sgx_remove(output_path);
    }
    return -1;
}

int ecall_show(const char *protected_file_path, const char *show_path) {
    SGX_FILE *protected_file = NULL;
    ssize_t len;
    int output_fd = 1; /* stdout */
    char buf[4 * 1024];

    protected_file = sgx_fopen_integrity_only(protected_file_path, "r");
    if (protected_file == NULL) {
        eprintf("Error: failed to open the given protected file %s\n", protected_file_path);
        goto on_error;
    }
    if (show_path) {
        output_fd = open_for_write(show_path);
        if (output_fd < 0) {
            eprintf("Error: failed to open the given show_path %s\n", show_path);
            goto on_error;
        }
    }

    while ((len = sgx_fread(buf, 1, sizeof(buf), protected_file)) > 0) {
        write(output_fd, buf, len);
    }

    if (sgx_ferror(protected_file)) {
        eprintf("Error: failed to read the given protected file %s\n", protected_file_path);
        goto on_error;
    }

    sgx_fclose(protected_file);
    if (output_fd > 1) {
        close(output_fd);
    }
    return 0;
on_error:
    if (protected_file != NULL) {
        sgx_fclose(protected_file);
    }
    if (output_fd > 1) {
        close(output_fd);
    }
    return -1;
}

int ecall_show_mac(const char *protected_file_path) {
    SGX_FILE *protected_file = NULL;
    sgx_aes_gcm_128bit_tag_t mac = { 0 };

    protected_file = sgx_fopen_integrity_only(protected_file_path, "r");
    if (protected_file == NULL) {
        eprintf("Error: failed to open the given protected file %s\n", protected_file_path);
        goto on_error;
    }

    if (sgx_fget_mac(protected_file, &mac)) {
        eprintf("Error: failed to get the MAC of the protected file %s\n", protected_file_path);
        goto on_error;
    }

    print_mac(&mac);

    sgx_fclose(protected_file);
    return 0;
on_error:
    if (protected_file != NULL) {
        sgx_fclose(protected_file);
    }
    return -1;
}
