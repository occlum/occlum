#ifndef __OCCLUM_PAL_API_H__
#define __OCCLUM_PAL_API_H__

#ifdef __cplusplus
extern "C" {
#endif

/*
 * @brief Initialize an Occlum enclave
 *
 * @param instance_dir  Specifies the path of an Occlum instance directory.
 *                      Usually, this directory is initialized by executing
 *                      "occlum init" command, which creates a hidden
 *                      directory named ".occlum/". This ".occlum/" is an
 *                      Occlum instance directory. The name of the directory is
 *                      not necesarrily ".occlum"; it can be renamed to an
 *                      arbitrary name.
 *
 * @retval If 0, then success; otherwise, check errno for the exact error type.
 */
int occlum_pal_init(const char* instance_dir);

/*
 * @brief Execute a command inside the Occlum enclave
 *
 * @param cmd_path      The path of the command to be executed
 * @param cmd_args      The arguments to the command. The array must be NULL
 *                      terminated.
 * @param exit_status   Output. The exit status of the command. Note that the
 *                      exit status is returned if and only if the function
 *                      succeeds.
 *
 * @retval If 0, then success; otherwise, check errno for the exact error type.
 */
int occlum_pal_exec(const char* cmd_path, const char** cmd_args, int* exit_status);

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
