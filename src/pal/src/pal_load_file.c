#include <stdio.h>
#include <stdlib.h>
#include "Enclave_u.h"
#include "pal_enclave.h"
#include "pal_error.h"
#include "pal_log.h"
#include "errno2str.h"

char *pal_load_file_to_string(const char *filename) {
    FILE *fp = fopen(filename, "rb");

    if (fp == NULL) {
        PAL_WARN("Warning: Failed to open file: %s", filename);
        return NULL;
    }
    fseek(fp, 0, SEEK_END);
    long fsize = ftell(fp);
    fseek(fp, 0, SEEK_SET);
    char *file_buffer = malloc(fsize + 1);
    if (file_buffer == NULL) {
        PAL_WARN("Warning: Failed to malloc buffer for file: %s", filename);
        return NULL;
    }
    fread(file_buffer, 1, fsize, fp);
    file_buffer[fsize] = '\0';
    fclose(fp);
    return file_buffer;
}

int pal_init_host_file(void) {
    sgx_enclave_id_t eid = pal_get_enclave_id();
    int ecall_ret = 0;

    sgx_status_t ecall_status = occlum_ecall_init_host_file(eid, &ecall_ret);
    if (ecall_status != SGX_SUCCESS) {
        const char *sgx_err = pal_get_sgx_error_msg(ecall_status);
        PAL_ERROR("Failed to do ECall with error code 0x%x: %s", ecall_status, sgx_err);
        return -1;
    }
    if (ecall_ret < 0) {
        errno = -ecall_ret;
        PAL_ERROR("occlum_ecall_init_host_file returns %s", errno2str(errno));
        return -1;
    }

    return 0;
}
