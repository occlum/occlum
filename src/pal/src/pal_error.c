#include "pal_error.h"

typedef struct {
    sgx_status_t err;
    const char *msg;
} sgx_err_msg_t;

static sgx_err_msg_t err_msg_table[] = {
    {
        SGX_SUCCESS,
        "SGX success."
    },
    {
        SGX_ERROR_UNEXPECTED,
        "Unexpected error occurred."
    },
    {
        SGX_ERROR_INVALID_PARAMETER,
        "Invalid parameter."
    },
    {
        SGX_ERROR_OUT_OF_MEMORY,
        "Out of memory."
    },
    {
        SGX_ERROR_ENCLAVE_LOST,
        "Power transition occurred."
    },
    {
        SGX_ERROR_INVALID_ENCLAVE,
        "Invalid enclave image."
    },
    {
        SGX_ERROR_INVALID_ENCLAVE_ID,
        "Invalid enclave identification."
    },
    {
        SGX_ERROR_INVALID_SIGNATURE,
        "Invalid enclave signature."
    },
    {
        SGX_ERROR_OUT_OF_EPC,
        "Out of EPC memory."
    },
    {
        SGX_ERROR_NO_DEVICE,
        "Invalid SGX device. Please make sure SGX module is enabled in the BIOS, and install SGX driver afterwards."
    },
    {
        SGX_ERROR_MEMORY_MAP_CONFLICT,
        "Memory map conflicted."
    },
    {
        SGX_ERROR_INVALID_METADATA,
        "Invalid enclave metadata."
    },
    {
        SGX_ERROR_DEVICE_BUSY,
        "SGX device was busy."
    },
    {
        SGX_ERROR_INVALID_VERSION,
        "Enclave version was invalid."
    },
    {
        SGX_ERROR_INVALID_ATTRIBUTE,
        "Enclave was not authorized."
    },
    {
        SGX_ERROR_ENCLAVE_FILE_ACCESS,
        "Can't open enclave file."
    },
    {
        SGX_ERROR_SERVICE_INVALID_PRIVILEGE,
        "Enclave has no privilege to get run in the release mode."
        "Please rebuild the Occlum enclave with a legal signing key "
        "(e.g., occlum build --sign-key <key_path>), "
        "to get a legal signing key, please contact Intel."
    },
};

const char *pal_get_sgx_error_msg(sgx_status_t error) {
    int err_max = sizeof err_msg_table / sizeof err_msg_table[0];
    for (int err_i = 0; err_i < err_max; err_i++) {
        if (error == err_msg_table[err_i].err) {
            return err_msg_table[err_i].msg;
        }
    }
    return "Unknown SGX error";
}
