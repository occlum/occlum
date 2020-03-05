#ifndef __OCCLUM_PAL_API_H__
#define __OCCLUM_PAL_API_H__

#ifdef __cplusplus
extern "C" {
#endif

/*
 * Occlum PAL attributes
 */
typedef struct {
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
struct occlum_stdio_fds {
    int stdin_fd;
    int stdout_fd;
    int stderr_fd;
};

/*
 * @brief Initialize an Occlum enclave
 *
 * @param attr  Mandatory input. Attributes for Occlum.
 *
 * @retval If 0, then success; otherwise, check errno for the exact error type.
 */
int occlum_pal_init(occlum_pal_attr_t* attr);

/*
 * @brief Execute a command inside the Occlum enclave
 *
 * @param cmd_path      The path of the command to be executed
 * @param cmd_args      The arguments to the command. The array must be NULL
 *                      terminated.
 * @param io_fds        The file descriptors of the redirected standard I/O
 *                      (i.e., stdin, stdout, stderr), If set to NULL, will
 *                      use the original standard I/O file descriptors.
 * @param exit_status   Output. The exit status of the command. Note that the
 *                      exit status is returned if and only if the function
 *                      succeeds.
 *
 * @retval If 0, then success; otherwise, check errno for the exact error type.
 */
int occlum_pal_exec(const char* cmd_path,
                    const char** cmd_args,
                    const struct occlum_stdio_fds* io_fds,
                    int* exit_status);

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
