#include <linux/limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <occlum_pal_api.h>

static const char* get_instance_dir(void) {
    const char* instance_dir_from_env = (const char*) getenv("OCCLUM_INSTANCE_DIR");
    if (instance_dir_from_env != NULL) {
        return instance_dir_from_env;
    }
    else {
        return "./.occlum";
    }
}

int main(int argc, char* argv[]) {
    // Parse arguments
    if (argc < 2) {
        fprintf(stderr, "[ERROR] occlum-run: at least one argument must be provided\n\n");
        fprintf(stderr, "Usage: occlum-run <executable> [<args>]\n");
        return EXIT_FAILURE;
    }
    const char* cmd_path = (const char*) argv[1];
    const char** cmd_args = (const char**) &argv[2];

    // Init Occlum PAL
    struct occlum_pal_attr attr = OCCLUM_PAL_ATTR_INITVAL;
    attr.instance_dir = get_instance_dir();
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
    int exit_status = 0;
    if (occlum_pal_exec(cmd_path, cmd_args, &io_fds, &exit_status) < 0) {
        return EXIT_FAILURE;
    }

    // Destroy Occlum PAL
    occlum_pal_destroy();

    return exit_status;
}
