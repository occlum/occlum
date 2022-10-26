#define _GNU_SOURCE
#include <occlum_pal_api.h>
#include "Enclave_u.h"
#include "pal_enclave.h"
#include "pal_error.h"
#include "pal_interrupt_thread.h"
#include "pal_timer_thread.h"
#include "pal_load_file.h"
#include "pal_log.h"
#include "pal_sig_handler.h"
#include "pal_syscall.h"
#include "pal_thread_counter.h"
#include "pal_vcpu_thread.h"
#include "pal_check_fsgsbase.h"
#include "errno2str.h"
#include <linux/limits.h>
#include <linux/futex.h>
#include <sys/syscall.h>
#include <unistd.h>
#include <sched.h>

#define MAX_NUM_VCPUS 1024

int occlum_pal_get_version(void) {
    return OCCLUM_PAL_VERSION;
}

int pal_run_init_process() {
    const char *init_path = "/bin/init";
    const char *init_argv[2] = {
        "init",
        NULL,
    };
    struct occlum_stdio_fds init_io_fds = {
        .stdin_fd = STDIN_FILENO,
        .stdout_fd = STDOUT_FILENO,
        .stderr_fd = STDERR_FILENO,
    };
    int libos_tid = 0;
    extern char **environ;
    volatile int exit_status = -1;
    struct occlum_pal_create_process_args init_process_args = {
        .path = init_path,
        .argv = init_argv,
        .env = environ,
        .stdio = &init_io_fds,
        .pid = &libos_tid,
        .exit_status = (int *) &exit_status,
    };
    if (occlum_pal_create_process(&init_process_args) < 0) {
        return -1;
    }

    int futex_val;
    while ((futex_val = exit_status) < 0) {
        (void)syscall(__NR_futex, &exit_status, FUTEX_WAIT, futex_val, NULL);
    }

    // Convert the exit status to a value in a shell-like encoding
    if (WIFEXITED(exit_status)) { // terminated normally
        exit_status = WEXITSTATUS(exit_status) & 0x7F; // [0, 127]
    } else { // killed by signal
        exit_status = 128 + WTERMSIG(exit_status); // [128 + 1, 128 + 64]
    }
    if (exit_status != 0) {
        errno = EINVAL;
        PAL_ERROR("The init process exit with code: %d", exit_status);
        return -1;
    }

    return 0;
}

int occlum_pal_get_available_cpu_count() {
    unsigned int count = 0;
    cpu_set_t set;
    // Check process cpu affinity
    if (sched_getaffinity(0, sizeof (set), &set) == 0) {
        count = CPU_COUNT(&set);
    }

    count = (count > 0 ) ? count : 1;
    count = (count < MAX_NUM_VCPUS) ? count : MAX_NUM_VCPUS;

    return count;
}

int occlum_pal_init(const struct occlum_pal_attr *attr) {
    if (attr == NULL) {
        errno = EINVAL;
        return -1;
    }
    if (attr->instance_dir == NULL) {
        errno = EINVAL;
        return -1;
    }

    char resolved_path[PATH_MAX] = {0};
    if (realpath(attr->instance_dir, resolved_path) == NULL) {
        PAL_ERROR("realpath returns %s", errno2str(errno));
        return -1;
    }

    // Check only for SGX hardware mode
#ifdef SGX_MODE_HW
    if (check_fsgsbase_enablement() != 0) {
        PAL_ERROR("FSGSBASE enablement check failed.");
        return -1;
    }
#endif

    if (attr->num_vcpus == 0 || attr->num_vcpus > MAX_NUM_VCPUS) {
        *(int *)(&attr->num_vcpus) = occlum_pal_get_available_cpu_count();
    }

    sgx_enclave_id_t eid = pal_get_enclave_id();
    if (eid != SGX_INVALID_ENCLAVE_ID) {
        PAL_ERROR("Enclave has been initialized.");
        errno = EEXIST;
        return -1;
    }

    if (pal_register_sig_handlers() < 0) {
        return -1;
    }

    if (pal_init_enclave(resolved_path) < 0) {
        return -1;
    }
    eid = pal_get_enclave_id();

    int ecall_ret = 0;
    struct host_file_buffer file_buffer = {
        .hostname_buf = pal_load_file_to_string("/etc/hostname"),
        .hosts_buf = pal_load_file_to_string("/etc/hosts"),
        .resolv_conf_buf = pal_load_file_to_string("/etc/resolv.conf"),
    };

    const struct host_file_buffer *file_buffer_ptr = &file_buffer;

    sgx_status_t ecall_status = occlum_ecall_init(eid, &ecall_ret, attr->log_level,
                                resolved_path, file_buffer_ptr, attr->num_vcpus);
    free_host_file_buffer(file_buffer);

    if (ecall_status != SGX_SUCCESS) {
        const char *sgx_err = pal_get_sgx_error_msg(ecall_status);
        PAL_ERROR("Failed to do ECall with error code 0x%x: %s", ecall_status, sgx_err);
        goto on_destroy_enclave;
    }
    if (ecall_ret < 0) {
        errno = -ecall_ret;
        PAL_ERROR("occlum_ecall_init returns %s", errno2str(errno));
        goto on_destroy_enclave;
    }

    if (pal_vcpu_threads_start(attr->num_vcpus) < 0) {
        PAL_ERROR("Failed to start the vCPU threads: %s", errno2str(errno));
        goto on_destroy_enclave;
    }

    if (pal_timer_thread_start() < 0) {
        PAL_ERROR("Failed to start the timer thread: %s", errno2str(errno));
        goto on_destroy_enclave;
    }

    if (pal_interrupt_thread_start() < 0) {
        PAL_ERROR("Failed to start the interrupt thread: %s", errno2str(errno));
        goto on_destroy_enclave;
    }

    if (pal_run_init_process() < 0) {
        PAL_ERROR("Failed to run the init process: %s", errno2str(errno));
        goto on_destroy_enclave;
    }

    return 0;
on_destroy_enclave:
    if (pal_destroy_enclave() < 0) {
        PAL_WARN("Cannot destroy the enclave");
    }
    return -1;
}

int occlum_pal_create_process(struct occlum_pal_create_process_args *args) {
    int ecall_ret = 0; // libos_tid

    if (args->path == NULL || args->argv == NULL || args->pid == NULL) {
        errno = EINVAL;
        return -1;
    }

    sgx_enclave_id_t eid = pal_get_enclave_id();
    if (eid == SGX_INVALID_ENCLAVE_ID) {
        PAL_ERROR("Enclave is not initialized yet.");
        errno = ENOENT;
        return -1;
    }

    sgx_status_t ecall_status = occlum_ecall_new_process(eid, &ecall_ret, args->path,
                                args->argv, args->env, args->stdio, args->exit_status);
    if (ecall_status != SGX_SUCCESS) {
        const char *sgx_err = pal_get_sgx_error_msg(ecall_status);
        PAL_ERROR("Failed to do ECall with error code 0x%x: %s", ecall_status, sgx_err);
        return -1;
    }
    if (ecall_ret < 0) {
        errno = -ecall_ret;
        PAL_ERROR("occlum_ecall_new_process returns %s", errno2str(errno));
        return -1;
    }

    *args->pid = ecall_ret;
    return 0;
}

int occlum_pal_run_vcpu(struct occlum_pal_vcpu_data *vcpu_ptr) {
    sgx_enclave_id_t eid = pal_get_enclave_id();
    if (eid == SGX_INVALID_ENCLAVE_ID) {
        PAL_ERROR("Enclave is not initialized yet.");
        errno = ENOENT;
        return -1;
    }

    int ecall_ret = 0;
    sgx_status_t ecall_status = occlum_ecall_run_vcpu(eid, &ecall_ret, vcpu_ptr);
    if (ecall_status != SGX_SUCCESS) {
        const char *sgx_err = pal_get_sgx_error_msg(ecall_status);
        PAL_ERROR("Failed to do ECall: %s", sgx_err);
        return -1;
    }
    if (ecall_ret < 0) {
        errno = -ecall_ret;
        PAL_ERROR("occlum_ecall_run_vcpu returns %s", errno2str(errno));
        return -1;
    }

    return 0;
}

int occlum_pal_kill(int pid, int sig) {
    sgx_enclave_id_t eid = pal_get_enclave_id();
    if (eid == SGX_INVALID_ENCLAVE_ID) {
        errno = ENOENT;
        PAL_ERROR("Enclave is not initialized yet.");
        return -1;
    }

    int ecall_ret = 0;
    sgx_status_t ecall_status = occlum_ecall_kill(eid, &ecall_ret, pid, sig);
    if (ecall_status != SGX_SUCCESS) {
        const char *sgx_err = pal_get_sgx_error_msg(ecall_status);
        PAL_ERROR("Failed to do ECall with error code 0x%x: %s", ecall_status, sgx_err);
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
    sgx_enclave_id_t eid = pal_get_enclave_id();
    if (eid == SGX_INVALID_ENCLAVE_ID) {
        PAL_ERROR("Enclave is not initialized yet.");
        errno = ENOENT;
        return -1;
    }

    int ret = 0;

    if (pal_vcpu_threads_stop() < 0) {
        ret = -1;
        PAL_WARN("Cannot stop the vCPU threads: %s", errno2str(errno));
    }

    if (pal_timer_thread_stop() < 0) {
        ret = -1;
        PAL_WARN("Cannot stop the timer thread: %s", errno2str(errno));
    }

    if (pal_interrupt_thread_stop() < 0) {
        ret = -1;
        PAL_WARN("Cannot stop the interrupt thread: %s", errno2str(errno));
    }

    // Make sure all helper threads exit
    int thread_counter;
    while ((thread_counter = pal_thread_counter_wait_zero(NULL)) > 0) ;

    // Make sure all helper threads exit
    if (pal_destroy_enclave() < 0) {
        ret = -1;
        PAL_WARN("Cannot destroy the enclave");
    }
    return ret;
}

void free_host_file_buffer(struct host_file_buffer file_buffer) {
    free((void *)file_buffer.hostname_buf);
    file_buffer.hostname_buf = NULL;

    free((void *)file_buffer.hosts_buf);
    file_buffer.hosts_buf = NULL;

    free((void *)file_buffer.resolv_conf_buf);
    file_buffer.resolv_conf_buf = NULL;
}

int pal_get_version(void) __attribute__((weak, alias ("occlum_pal_get_version")));

int pal_init(const struct occlum_pal_attr *attr)\
__attribute__ ((weak, alias ("occlum_pal_init")));

int pal_create_process(struct occlum_pal_create_process_args *args)\
__attribute__ ((weak, alias ("occlum_pal_create_process")));

int pal_kill(int pid, int sig) __attribute__ ((weak, alias ("occlum_pal_kill")));

int pal_destroy(void) __attribute__ ((weak, alias ("occlum_pal_destroy")));
