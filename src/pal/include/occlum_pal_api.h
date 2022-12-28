#ifndef __OCCLUM_PAL_API_H__
#define __OCCLUM_PAL_API_H__

#ifdef __cplusplus
extern "C" {
#endif

/*
 * Occlum PAL API version number
 */
#define OCCLUM_PAL_VERSION 2

/*
 * @brief Get version of Occlum PAL API
 *
 * @retval If > 0, then success; otherwise, it is an invalid version.
 */
int occlum_pal_get_version(void);

/*
 * Occlum PAL attributes
 */
typedef struct occlum_pal_attr {
    // Occlum instance directory.
    //
    // Specifies the path of an Occlum instance directory, which is usually created with the
    // `occlum new` command. The default value is "."; that is, the current working directory
    // is the Occlum instance directory.
    const char     *instance_dir;
    // Log level.
    //
    // Specifies the log level of Occlum LibOS. Valid values: "off", "error",
    // "warn", "info", and "trace". Case insensitive.
    //
    // Optional field. If NULL, the LibOS will treat it as "off".
    const char     *log_level;
    // The number of vCPUs. A value of zero means using as many as CPU cores
    // on the platform.
    unsigned int    num_vcpus;
} occlum_pal_attr_t;

#define OCCLUM_PAL_ATTR_INITVAL         { \
    .instance_dir = ".",                  \
    .log_level = NULL,                    \
    .num_vcpus = 0                        \
}

/*
 * The struct which consists of file descriptors of standard I/O
 */
typedef struct occlum_stdio_fds {
    int stdin_fd;
    int stdout_fd;
    int stderr_fd;
} occlum_stdio_fds_t;

struct host_file_buffer {
    const char *resolv_conf_buf;
    const char *hosts_buf;
    const char *hostname_buf;
};

/*
 * The struct which consists of arguments needed by occlum_pal_create_process
 */
struct occlum_pal_create_process_args {

    // Path to new process.
    //
    // The path of the command which will be created as a new process.
    //
    // Mandatory field. Must not be NULL.
    const char *path;

    // Arguments array pass to new process.
    //
    // The arguments to the command. By convention, the argv[0] should be the program name.
    // And the array must be NULL terminated.
    //
    // Mandatory field. Must not be NULL.
    const char **argv;

    // Untrusted environment variable array pass to new process.
    //
    // The untrusted env vars to the command. The array must be NULL terminated.
    //
    // Optional field.
    const char **env;

    // File descriptors of the redirected standard I/O (i.e., stdin, stdout, stderr)
    //
    // If set to NULL, will use the original standard I/O file descriptors.
    //
    // Optional field.
    const struct occlum_stdio_fds *stdio;

    // Output. Pid of new process in libos.
    //
    // If occlum_pal_create_process returns success, pid of the new process will
    // be updated.
    //
    // Mandatory field. Must not be NULL.
    int *pid;

    // Output. Will be set when the process terminates. Also a call of futex
    // wake will be made on the address so that a user can futex wait on the
    // address.
    int *exit_status;
};

struct occlum_pal_vcpu_data {
    // Output. Mark to indicate if the enclave threads running in the VCPU is in user space
    // Set to 1 if entering to user space.
    // Set to 0 if exited from user space.
    // Only do sig64 kill when VCPU is running in user space.
    unsigned int user_space_mark;
};

/*
 * @brief Initialize an Occlum enclave
 *
 * @param attr  Mandatory input. Attributes for Occlum.
 *
 * @retval If 0, then success; otherwise, check errno for the exact error type.
 */
int occlum_pal_init(const struct occlum_pal_attr *attr);

/*
 * @brief Create a new process inside the Occlum enclave
 *
 * @param args  Mandatory input. Arguments for occlum_pal_create_process.
 *
 * @retval If 0, then success; otherwise, check errno for the exact error type.
 */
int occlum_pal_create_process(struct occlum_pal_create_process_args *args);


int occlum_pal_run_vcpu(struct occlum_pal_vcpu_data *vcpu_ptr);


/*
 * @brief Send a signal to one or multiple LibOS processes
 *
 * @param pid   If pid > 0, send the signal to the process with the
 *              pid; if pid == -1, send the signal to all processes.
 * @param sig   The signal number. For the purpose of security, the
 *              only allowed signals for now are SIGKILL and SIGTERM.
 *
 * @retval If 0, then success; otherwise, check errno for the exact error type.
 */
int occlum_pal_kill(int pid, int sig);

/*
 * @brief Destroy teh Occlum enclave
 *
 * @retval if 0, then success; otherwise, check errno for the exact error type.
 */
int occlum_pal_destroy(void);

void free_host_file_buffer(struct host_file_buffer file_buffer);

#ifdef __cplusplus
}
#endif

#endif /* __OCCLUM_PAL_API_H__ */
