#include <linux/limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <occlum_pal_api.h>

//============================================================================
// Help message
//============================================================================

#define HELP_MSG                                                                    \
    "%s\n"                                                                          \
    "A benchmark program that measures the memory throughput across the enclave.\n" \
    "\n"                                                                            \
    "Usage:\n"                                                                      \
    "    %s <total_bytes>\n"                                                        \
    "\n"                                                                            \
    "Arguments:\n"                                                                  \
    "    <total_bytes>      The total number of bytes that are copied from the outside of an enclave to the inside" \
    "\n"                                                                            \
    "Note:\n"                                                                       \
    "    This simple benchmark program showcases the power of the embedded mode of Occlum, " \
    "which enables sharing memory between the inside and outside of an enclave."    \
    "The embedded mode makes it possible to build Occlum-based SGX apps "           \
    "that comprise of trusted and untrused halves.\n"

static void print_help_msg(const char *prog_name) {
    fprintf(stderr, HELP_MSG, prog_name, prog_name);
}

//============================================================================
// Main
//============================================================================

int main(int argc, char *argv[]) {
    // Parse arguments
    const char *prog_name = (const char *)argv[0];
    if (argc < 2) {
        fprintf(stderr, "error: require one argument\n\n");
        print_help_msg(prog_name);
        return EXIT_FAILURE;
    }
    const char *total_bytes_str = argv[1];

    // Init Occlum PAL
    occlum_pal_attr_t pal_attr = OCCLUM_PAL_ATTR_INITVAL;
    pal_attr.instance_dir = "occlum_instance";
    pal_attr.log_level = "off";
    if (occlum_pal_init(&pal_attr) < 0) {
        return EXIT_FAILURE;
    }

    // The buffer shared between the outside and inside the enclave
    char shared_buf[1024 * 1024] = {0};

    // Prepare cmd path and arguments
    const char *cmd_path = "/bin/trusted_memcpy_bench";
    char buf_ptr_str[32] = {0};
    char buf_size_str[32] = {0};
    snprintf(buf_ptr_str, sizeof buf_ptr_str, "%lu", (unsigned long) shared_buf);
    snprintf(buf_size_str, sizeof buf_size_str, "%lu", sizeof shared_buf);
    const char *cmd_args[] = {
        cmd_path,
        buf_ptr_str, // buf_ptr
        buf_size_str, // buf_size
        total_bytes_str, // total_bytes
        NULL
    };

    struct occlum_stdio_fds io_fds = {
        .stdin_fd = STDIN_FILENO,
        .stdout_fd = STDOUT_FILENO,
        .stderr_fd = STDERR_FILENO,
    };

    // Use Occlum PAL to create new process
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

    // Use Occlum PAL to execute the cmd
    int exit_status = 0;
    struct occlum_pal_exec_args exec_args = {
        .pid = libos_tid,
        .exit_value = &exit_status,
    };
    if (occlum_pal_exec(&exec_args) < 0) {
        return EXIT_FAILURE;
    }

    // Destroy Occlum PAL
    occlum_pal_destroy();

    return exit_status;
}
