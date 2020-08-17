#include <stdio.h>
#include <sched.h>
#include <sys/sysinfo.h>
#include <unistd.h>
#include <linux/limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <pthread.h>
#include <errno.h>
#include <occlum_pal_api.h>
#include "sgx_eid.h"
#include "sgx_urts.h"

#include "EnclaveInitiator_u.h"

#define ENCLAVE_INITIATOR_NAME "./libenclave_initiator.signed.so"

pthread_t thread;
sgx_enclave_id_t initiator_enclave_id = 0;
void *attestation(void *arg);

int main(int argc, char *argv[]) {
    int update = 0;
    sgx_launch_token_t token = {0};
    sgx_status_t status;
    int exit_status = 0;
    const char *cmd_path = "/bin/responder"; // Prepare cmd path and arguments
    const char *cmd_args[] = {NULL};

    // create ECDH initiator enclave
    status = sgx_create_enclave(ENCLAVE_INITIATOR_NAME, SGX_DEBUG_FLAG, &token, &update,
                                &initiator_enclave_id, NULL);
    if (status != SGX_SUCCESS) {
        printf("failed to load enclave %s, error code is 0x%x.\n", ENCLAVE_INITIATOR_NAME,
               status);
        return -1;
    }
    printf("succeed to load enclave %s\n", ENCLAVE_INITIATOR_NAME);

    occlum_pal_attr_t attr = OCCLUM_PAL_ATTR_INITVAL;
    attr.log_level = (const char *) getenv("OCCLUM_LOG_LEVEL");
    if (occlum_pal_init(&attr) < 0) {
        return EXIT_FAILURE;
    }

    if (pthread_create(&thread, NULL, attestation, NULL) < 0) {
        printf("pthread_create failed\n");
        return -1;
    }

    // Use Occlum PAL to create new process for the responder
    struct occlum_stdio_fds io_fds = {
        .stdin_fd = STDIN_FILENO,
        .stdout_fd = STDOUT_FILENO,
        .stderr_fd = STDERR_FILENO,
    };

    int libos_tid = 0;
    struct occlum_pal_create_process_args create_process_args = {
        .path = cmd_path,
        .argv = cmd_args,
        .env = NULL,
        .stdio = (const struct occlum_stdio_fds *) &io_fds,
        .pid = &libos_tid,
    };

    if (occlum_pal_create_process(&create_process_args) < 0) {
        return EXIT_FAILURE;
    }

    // execute the responder process
    struct occlum_pal_exec_args exec_args = {
        .pid = libos_tid,
        .exit_value = &exit_status,
    };
    if (occlum_pal_exec(&exec_args) < 0) {
        return EXIT_FAILURE;
    }

    // wait for end and destroy
    if (pthread_join(thread, NULL) < 0) {
        printf("pthread_join failed\n");
        return -1;
    }

    status = sgx_destroy_enclave(initiator_enclave_id);
    if (status != SGX_SUCCESS) {
        printf("failed to destroy enclave %s, error code is 0x%x.\n", ENCLAVE_INITIATOR_NAME,
               status);
        return -1;
    }

    if (occlum_pal_destroy() < 0) {
        printf("occlum_pal_destroy failed, errno is %d\n", errno);
        return -1;
    }

    printf("Local attestation Sucess!\n");
    return 0;
}

// create ECDH session using initiator enclave
// it would create ECDH session with responder enclave running in another process
void *attestation(void *arg) {
    sgx_status_t status;
    uint32_t ret_status;

    sleep(5);
    status = test_create_session(initiator_enclave_id, &ret_status);
    if (status != SGX_SUCCESS || ret_status != 0) {
        printf("failed to establish secure channel: ECALL return 0x%x, error code is 0x%x.\n",
               status, ret_status);
        return NULL;
    }
    printf("succeed to establish secure channel.\n");

    status = test_message_exchange(initiator_enclave_id, &ret_status);
    if (status != SGX_SUCCESS || ret_status != 0) {
        printf("test_message_exchange Ecall failed: ECALL return 0x%x, error code is 0x%x.\n",
               status, ret_status);
        sgx_destroy_enclave(initiator_enclave_id);
        return NULL;
    }
    printf("Succeed to exchange secure message.\n");

    // close ECDH session
    status = test_close_session(initiator_enclave_id, &ret_status);
    if (status != SGX_SUCCESS || ret_status != 0) {
        printf("test_close_session Ecall failed: ECALL return 0x%x, error code is 0x%x.\n",
               status, ret_status);
        return NULL;
    }
    printf("Succeed to close session.\n");
    pthread_exit(NULL);
}
