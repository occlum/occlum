#include <stdio.h>
#include <stdlib.h>
#include "Enclave_u.h"
#include "pal_log.h"
#include "pal_load_file.h"

void pal_load_file(const sgx_enclave_id_t eid, const char *filename,
                   load_file_t *load_file) {
#ifndef SGX_MODE_HYPER
    UNUSED(eid);
#endif
    FILE *fp = fopen(filename, "rb");

    if (fp == NULL) {
        PAL_WARN("Warning: Failed to open file: %s", filename);
        return;
    }
    fseek(fp, 0, SEEK_END);
    long fsize = ftell(fp);

    fseek(fp, 0, SEEK_SET);
#ifndef SGX_MODE_HYPER
    load_file->buffer = malloc(fsize + 1);
#else
    load_file->buffer = sgx_ecall_ms_buffer_alloc(eid, fsize + 1);
#endif
    if (load_file->buffer == NULL) {
        PAL_WARN("Warning: Failed to malloc buffer for file: %s", filename);
        return;
    }
    fread(load_file->buffer, 1, fsize, fp);
    load_file->buffer[fsize] = '\0';
    load_file->size = fsize + 1;

    fclose(fp);
}

void free_host_file_buffer(const sgx_enclave_id_t eid,
                           struct host_file_buffer_t *file_buffer) {
#ifndef SGX_MODE_HYPER
    UNUSED(eid);
    if (file_buffer->hostname_buf) {
        free((void *)file_buffer->hostname_buf);
    }
    if (file_buffer->hosts_buf) {
        free((void *)file_buffer->hosts_buf);
    }
    if (file_buffer->resolv_conf_buf) {
        free((void *)file_buffer->resolv_conf_buf);
    }
#else
    if (file_buffer->hostname_buf) {
        sgx_ecall_ms_buffer_free(eid);
    }
    if (file_buffer->hosts_buf) {
        sgx_ecall_ms_buffer_free(eid);
    }
    if (file_buffer->resolv_conf_buf) {
        sgx_ecall_ms_buffer_free(eid);
    }
#endif
    file_buffer->hostname_buf = NULL;
    file_buffer->hostname_buf_size = 0;
    file_buffer->hosts_buf = NULL;
    file_buffer->hosts_buf_size = 0;
    file_buffer->resolv_conf_buf = NULL;
    file_buffer->resolv_conf_buf_size = 0;
}
