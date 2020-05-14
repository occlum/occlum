#ifndef __OCCLUM_PAL_API_H__
#define __OCCLUM_PAL_API_H__

#ifdef __cplusplus
extern "C" {
#endif

/*
 * Occlum PAL API version number
 */
#define OCCLUM_PAL_VERSION 1

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
    // Occlum instance dir.
    //
    // Specifies the path of an Occlum instance directory. Usually, this
    // directory is initialized by executing "occlum init" command, which
    // creates a hidden directory named ".occlum/". This ".occlum/" is an
    // Occlum instance directory. The name of the directory is not necesarrily
    // ".occlum"; it can be renamed to an arbitrary name.
    //
    // Mandatory field. Must not be NULL.
    const char*     instance_dir;
    // Log level.
    //
    // Specifies the log level of Occlum LibOS. Valid values: "off", "error",
    // "warn", "info", and "trace". Case insensitive.
    //
    // Optional field. If NULL, the LibOS will treat it as "off".
    const char*     log_level;
} occlum_pal_attr_t;

#define OCCLUM_PAL_ATTR_INITVAL         { \
    .instance_dir = NULL,                 \
    .log_level = NULL                     \
}

/*
 * The struct which consists of file descriptors of standard I/O
 */
typedef struct occlum_stdio_fds {
    int stdin_fd;
    int stdout_fd;
    int stderr_fd;
} occlum_stdio_fds_t;

/*
 * @brief Initialize an Occlum enclave
 *
 * @param attr  Mandatory input. Attributes for Occlum.
 *
 * @retval If 0, then success; otherwise, check errno for the exact error type.
 */
int occlum_pal_init(const struct occlum_pal_attr* attr);

/*
 * @brief Execute a command inside the Occlum enclave
 *
 * @param cmd_path      The path of the command to be executed
 * @param cmd_args      The arguments to the command. The array must be NULL
 *                      terminated.
 * @param io_fds        The file descriptors of the redirected standard I/O
 *                      (i.e., stdin, stdout, stderr), If set to NULL, will
 *                      use the original standard I/O file descriptors.
 * @param exit_status   Output. The exit status of the command. The semantic of
 *                      this value follows the one described in wait(2) man
 *                      page. For example, if the program terminated normally,
 *                      then WEXITSTATUS(exit_status) gives the value returned
 *                      from a main function.
 *
 * @retval If 0, then success; otherwise, check errno for the exact error type.
 */
int occlum_pal_exec(const char* cmd_path,
                    const char** cmd_args,
                    const struct occlum_stdio_fds* io_fds,
                    int* exit_status);

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

#ifdef __cplusplus
}
#endif

#endif /* __OCCLUM_PAL_API_H__ */
