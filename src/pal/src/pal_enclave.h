#ifndef __PAL_ENCLAVE_H__
#define __PAL_ENCLAVE_H__

#include <sgx_eid.h>

int pal_init_enclave(const char *instance_dir);
int pal_destroy_enclave(void);

#define SGX_INVALID_ENCLAVE_ID          (-1)
sgx_enclave_id_t pal_get_enclave_id(void);

#endif /* __PAL_ENCLAVE_H__ */
