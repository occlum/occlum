#include <linux/limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <string.h>
#include <libgen.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <occlum_pal_api.h>
#include <sys/prctl.h>
#include <sys/syscall.h>
#include <linux/futex.h>

#define FUTEX_WAIT_TIMEOUT(addr, val, timeout)  ((int)syscall(__NR_futex, (addr), FUTEX_WAIT, (val), (timeout)))

int main(int argc, char *argv[]) {
    // Parse arguments
    if (argc < 2) {
        fprintf(stderr, "[ERROR] occlum-run: at least one argument must be provided\n\n");
        fprintf(stderr, "Usage: occlum-run <executable> [<args>]\n");
        return EXIT_FAILURE;
    }

    char **cmd_args = &argv[1];
    char *cmd_path = strdup(argv[1]);
    extern const char **environ;

    // Change cmd_args[0] from program path to program name in place (e.g., "/bin/abc" to "abc")
    char *cmd_path_tmp = strdup(cmd_path);
    const char *program_name = (const char *) basename(cmd_path_tmp);
    memset(cmd_args[0], 0, strlen(cmd_args[0]));
    memcpy(cmd_args[0], program_name, strlen(program_name));

    // Check Occlum PAL version
    int pal_version = occlum_pal_get_version();
    if (pal_version <= 0) {
        return EXIT_FAILURE;
    }

    // Init Occlum PAL
    struct occlum_pal_attr attr = OCCLUM_PAL_ATTR_INITVAL;
    attr.log_level = getenv("OCCLUM_LOG_LEVEL");
    if (occlum_pal_init(&attr) < 0) {
        return EXIT_FAILURE;
    }

    // Use Occlum PAL to execute the cmd
    struct occlum_stdio_fds io_fds = {
        .stdin_fd = STDIN_FILENO,
        .stdout_fd = STDOUT_FILENO,
        .stderr_fd = STDERR_FILENO,
    };
    int libos_tid = 0;
    volatile int exit_status = -1;
    struct occlum_pal_create_process_args create_process_args = {
        .path = (const char *) cmd_path,
        .argv = (const char **) cmd_args,
        .env = environ,
        .stdio = (const struct occlum_stdio_fds *) &io_fds,
        .pid = &libos_tid,
        .exit_status = (int *) &exit_status,
    };
    if (occlum_pal_create_process(&create_process_args) < 0) {
        // Command not found or other internal errors
        return 127;
    }

    int futex_val;
    while ((futex_val = exit_status) < 0) {
        (void)FUTEX_WAIT_TIMEOUT(&exit_status, futex_val, NULL);
    }

    // Convert the exit status to a value in a shell-like encoding
    if (WIFEXITED(exit_status)) { // terminated normally
        exit_status = WEXITSTATUS(exit_status) & 0x7F; // [0, 127]
    } else { // killed by signal
        exit_status = 128 + WTERMSIG(exit_status); // [128 + 1, 128 + 64]
    }

    // Destroy Occlum PAL
    occlum_pal_destroy();

    return exit_status;
}
