#include <stdlib.h>
#include <pthread.h>
#include "ocalls.h"

typedef struct {
    sgx_enclave_id_t    enclave_id;
    int                 libos_tid;
} thread_data_t;

void* exec_libos_thread(void* _thread_data) {
    thread_data_t* thread_data = _thread_data;
    sgx_enclave_id_t eid = thread_data->enclave_id;
    int host_tid = gettid();
    int libos_tid = thread_data->libos_tid;
    int libos_exit_status = -1;
    sgx_status_t status = occlum_ecall_exec_thread(eid, &libos_exit_status, libos_tid, host_tid);
    if (status != SGX_SUCCESS) {
        const char* sgx_err = pal_get_sgx_error_msg(status);
        PAL_ERROR("Failed to enter the enclave to execute a LibOS thread: %s", sgx_err);
        exit(EXIT_FAILURE);
    }

    free(thread_data);
    return NULL;
}

// Start a new host OS thread and enter the enclave to execute the LibOS thread
int occlum_ocall_exec_thread_async(int libos_tid) {
    int ret = 0;
    pthread_t thread;

    thread_data_t* thread_data = malloc(sizeof *thread_data);
    thread_data->enclave_id = pal_get_enclave_id();
    thread_data->libos_tid = libos_tid;

    if ((ret = pthread_create(&thread, NULL, exec_libos_thread, thread_data)) < 0) {
        free(thread_data);
        return -1;
    }
    pthread_detach(thread);

    // Note: thread_data is freed just before the thread exits

    return 0;
}
