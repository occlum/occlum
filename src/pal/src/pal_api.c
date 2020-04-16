#include <occlum_pal_api.h>
#include "Enclave_u.h"
#include "pal_enclave.h"
#include "pal_error.h"
#include "pal_log.h"
#include "pal_syscall.h"

int occlum_pal_init(const struct occlum_pal_attr* attr) {
    errno = 0;

    if (attr == NULL) {
        errno = EINVAL;
        return -1;
    }
    if (attr->instance_dir == NULL) {
        errno = EINVAL;
        return -1;
    }

    sgx_enclave_id_t eid = pal_get_enclave_id();
    if (eid != SGX_INVALID_ENCLAVE_ID) {
        PAL_ERROR("Enclave has been initialized.");
        errno = EEXIST;
        return -1;
    }

    if (pal_init_enclave(attr->instance_dir) < 0) {
        return -1;
    }

    // Invoke a do-nothing ECall for two purposes:
    // 1) Test the enclave can work;
    // 2) Initialize the global data structures inside the enclave (which is
    // automatically done by Intel SGX SDK).
    eid = pal_get_enclave_id();
    int ret;
    sgx_status_t ecall_status = occlum_ecall_init(eid, &ret, attr->log_level);
    if (ecall_status != SGX_SUCCESS) {
        const char* sgx_err = pal_get_sgx_error_msg(ecall_status);
        PAL_ERROR("Failed to do ECall: %s", sgx_err);
        return -1;
    }
    if (ret < 0) {
        errno = EINVAL;
        return -1;
    }
    return 0;
}

int occlum_pal_exec(const char* cmd_path,
                    const char** cmd_args,
                    const struct occlum_stdio_fds* io_fds,
                    int* exit_status) {
    errno = 0;

    if (cmd_path == NULL || cmd_args == NULL || exit_status == NULL) {
        errno = EINVAL;
        return -1;
    }

    sgx_enclave_id_t eid = pal_get_enclave_id();
    if (eid == SGX_INVALID_ENCLAVE_ID) {
        PAL_ERROR("Enclave is not initialized yet.");
        errno = ENOENT;
        return -1;
    }

    int libos_tid = -1;
    sgx_status_t ecall_status = occlum_ecall_new_process(eid, &libos_tid, cmd_path, cmd_args, io_fds);
    if (ecall_status != SGX_SUCCESS) {
        const char* sgx_err = pal_get_sgx_error_msg(ecall_status);
        PAL_ERROR("Failed to do ECall: %s", sgx_err);
        return -1;
    }
    if (libos_tid < 0) {
        return -1;
    }

    int host_tid = gettid();
    ecall_status = occlum_ecall_exec_thread(eid, exit_status, libos_tid, host_tid);
    if (ecall_status != SGX_SUCCESS) {
        const char* sgx_err = pal_get_sgx_error_msg(ecall_status);
        PAL_ERROR("Failed to do ECall: %s", sgx_err);
        return -1;
    }

    return 0;
}

int occlum_pal_destroy(void) {
    errno = 0;

    sgx_enclave_id_t eid = pal_get_enclave_id();
    if (eid == SGX_INVALID_ENCLAVE_ID) {
        PAL_ERROR("Enclave is not initialized yet.");
        errno = ENOENT;
        return -1;
    }

    if (pal_destroy_enclave() < 0) {
        return -1;
    }
    return 0;
}
