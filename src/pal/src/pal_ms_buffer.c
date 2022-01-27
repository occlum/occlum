#ifdef SGX_MODE_HYPER
#include <stdlib.h>
#include <string.h>
#include <sgx_eid.h>

#include "Enclave_u.h"
#include "pal_ms_buffer.h"

void ms_buffer_string_array_free(sgx_enclave_id_t eid, const char **str_array) {
    if (!str_array) {
        return;
    }

    const char *string = *str_array;
    int array_size = 0;

    while (string) {
        sgx_ecall_ms_buffer_free(eid);
        array_size++;
        string = str_array[array_size];
    }
    sgx_ecall_ms_buffer_free(eid);
}

const char **ms_buffer_convert_string_array(sgx_enclave_id_t eid,
        const char **str_array) {
    if (str_array == NULL) {
        return NULL;
    }

    int string_len = 0;
    const char *string = *str_array;
    int array_size = 0;

    while (string) {
        array_size++;
        string = str_array[array_size];
    }

    const char **ms_buf_str_array = (const char **)sgx_ecall_ms_buffer_alloc(eid,
                                    sizeof(char *) * (array_size + 1));

    if (!ms_buf_str_array) {
        return NULL;
    }

    for (int i = 0; i < array_size; ++i) {
        ms_buf_str_array[i] = NULL;
        string = str_array[i];
        string_len = strlen(string);

        char *ms_parameter = (char *)sgx_ecall_ms_buffer_alloc(eid, string_len + 1);

        if (!ms_parameter) {
            ms_buffer_string_array_free(eid, ms_buf_str_array);
            return NULL;
        }

        memcpy(ms_parameter, string, string_len);
        ms_parameter[string_len] = 0;

        ms_buf_str_array[i] = ms_parameter;
    }

    ms_buf_str_array[array_size] = NULL;
    return ms_buf_str_array;
}
#endif //SGX_MODE_HYPER
