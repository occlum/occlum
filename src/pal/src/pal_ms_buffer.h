#ifndef __PAL_MS_BUFFER_H__
#define __PAL_MS_BUFFER_H__

#ifdef SGX_MODE_HYPER
#include <sgx_eid.h>

const char **ms_buffer_convert_string_array(sgx_enclave_id_t eid, const char **str_array);
void ms_buffer_string_array_free(sgx_enclave_id_t eid, const char **str_array);
#endif

#endif /* __PAL_MS_BUFFER_H__ */
