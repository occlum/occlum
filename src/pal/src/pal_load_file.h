#ifndef __PAL_LOAD_FILE_H__
#define __PAL_LOAD_FILE_H__
#include <sgx_eid.h>

#define UNUSED(val) (void)(val)

typedef struct {
    unsigned int size;
    char *buffer;
} load_file_t;

void pal_load_file(const sgx_enclave_id_t eid, const char *filename,
                   load_file_t *load_file);
void free_host_file_buffer(const sgx_enclave_id_t eid,
                           struct host_file_buffer_t *file_buffer);

#endif /* __PAL_LOAD_FILE_H__ */
