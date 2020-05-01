#include <occlum_pal_api.h>
#include "Enclave_u.h"
#include "pal_enclave.h"
#include "pal_error.h"
#include "pal_log.h"
#include "pal_syscall.h"
#include "errno2str.h"

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
    eid = pal_get_enclave_id();

    int ecall_ret = 0;
    sgx_status_t ecall_status = occlum_ecall_init(eid, &ecall_ret, attr->log_level, attr->instance_dir);
    if (ecall_status != SGX_SUCCESS) {
        const char* sgx_err = pal_get_sgx_error_msg(ecall_status);
        PAL_ERROR("Failed to do ECall: %s", sgx_err);
        return -1;
    }
    if (ecall_ret < 0) {
        errno = -ecall_ret;
        PAL_ERROR("occlum_ecall_init returns %s", errno2str(errno));
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

    int ecall_ret = 0; // libos_tid
    sgx_status_t ecall_status = occlum_ecall_new_process(eid, &ecall_ret, cmd_path, cmd_args, io_fds);
    if (ecall_status != SGX_SUCCESS) {
        const char* sgx_err = pal_get_sgx_error_msg(ecall_status);
        PAL_ERROR("Failed to do ECall: %s", sgx_err);
        return -1;
    }
    if (ecall_ret < 0) {
        errno = -ecall_ret;
        PAL_ERROR("occlum_ecall_new_process returns %s", errno2str(errno));
        return -1;
    }

    int libos_tid = ecall_ret;
    int host_tid = gettid();
    ecall_status = occlum_ecall_exec_thread(eid, &ecall_ret, libos_tid, host_tid);
    if (ecall_status != SGX_SUCCESS) {
        const char* sgx_err = pal_get_sgx_error_msg(ecall_status);
        PAL_ERROR("Failed to do ECall: %s", sgx_err);
        return -1;
    }
    if (ecall_ret < 0) {
        errno = -ecall_ret;
        PAL_ERROR("occlum_ecall_exec_thread returns %s", errno2str(errno));
        return -1;
    }

    *exit_status = ecall_ret;
    return 0;
}

int occlum_pal_kill(int pid, int sig) {
    errno = 0;

    sgx_enclave_id_t eid = pal_get_enclave_id();
    if (eid == SGX_INVALID_ENCLAVE_ID) {
        errno = ENOENT;
        PAL_ERROR("Enclave is not initialized yet.");
        return -1;
    }

    int ecall_ret = 0;
    sgx_status_t ecall_status = occlum_ecall_kill(eid, &ecall_ret, pid, sig);
    if (ecall_status != SGX_SUCCESS) {
        const char* sgx_err = pal_get_sgx_error_msg(ecall_status);
        PAL_ERROR("Failed to do ECall: %s", sgx_err);
        return -1;
    }
    if (ecall_ret < 0) {
        errno = -ecall_ret;
        PAL_ERROR("Failed to occlum_ecall_kill: %s", errno2str(errno));
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
