#include "Enclave_u.h"

#include <assert.h>
#include <fcntl.h>
#include <libgen.h>
#include <pwd.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/time.h>
#include <sys/types.h>
#include <unistd.h>

#include <sgx_eid.h>
#include <sgx_error.h>
#include <sgx_urts.h>

#include "../../../src/pal/include/occlum_version.h"

#define MAX_PATH            FILENAME_MAX
#define TOKEN_FILENAME      "enclave.token"
#define ENCLAVE_FILENAME    "occlum-protect-integrity.signed.so." STRINGIZE(OCCLUM_MAJOR_VERSION)

// ==========================================================================
//  Enclave Initialization
// ==========================================================================

/* Global EID shared by multiple threads */
static sgx_enclave_id_t global_eid = 0;

typedef struct _sgx_errlist_t {
    sgx_status_t err;
    const char *msg;
    const char *sug; /* Suggestion */
} sgx_errlist_t;

/* Error code returned by sgx_create_enclave */
static sgx_errlist_t sgx_errlist[] = {
    {
        SGX_ERROR_UNEXPECTED,
        "Unexpected error occurred.",
        NULL
    },
    {
        SGX_ERROR_INVALID_PARAMETER,
        "Invalid parameter.",
        NULL
    },
    {
        SGX_ERROR_OUT_OF_MEMORY,
        "Out of memory.",
        NULL
    },
    {
        SGX_ERROR_ENCLAVE_LOST,
        "Power transition occurred.",
        "Please refer to the sample \"PowerTransition\" for details."
    },
    {
        SGX_ERROR_INVALID_ENCLAVE,
        "Invalid enclave image.",
        NULL
    },
    {
        SGX_ERROR_INVALID_ENCLAVE_ID,
        "Invalid enclave identification.",
        NULL
    },
    {
        SGX_ERROR_INVALID_SIGNATURE,
        "Invalid enclave signature.",
        NULL
    },
    {
        SGX_ERROR_OUT_OF_EPC,
        "Out of EPC memory.",
        NULL
    },
    {
        SGX_ERROR_NO_DEVICE,
        "Invalid SGX device.",
        "Please make sure SGX module is enabled in the BIOS, and install SGX driver afterwards."
    },
    {
        SGX_ERROR_MEMORY_MAP_CONFLICT,
        "Memory map conflicted.",
        NULL
    },
    {
        SGX_ERROR_INVALID_METADATA,
        "Invalid enclave metadata.",
        NULL
    },
    {
        SGX_ERROR_DEVICE_BUSY,
        "SGX device was busy.",
        NULL
    },
    {
        SGX_ERROR_INVALID_VERSION,
        "Enclave version was invalid.",
        NULL
    },
    {
        SGX_ERROR_INVALID_ATTRIBUTE,
        "Enclave was not authorized.",
        NULL
    },
    {
        SGX_ERROR_ENCLAVE_FILE_ACCESS,
        "Can't open enclave file.",
        NULL
    },
};

/* Check error conditions for loading enclave */
static void print_error_message(sgx_status_t ret) {
    size_t idx = 0;
    size_t ttl = sizeof sgx_errlist / sizeof sgx_errlist[0];

    for (idx = 0; idx < ttl; idx++) {
        if (ret == sgx_errlist[idx].err) {
            if (NULL != sgx_errlist[idx].sug) {
                printf("Info: %s\n", sgx_errlist[idx].sug);
            }
            printf("Error: %s\n", sgx_errlist[idx].msg);
            break;
        }
    }

    if (idx == ttl) {
        printf("Error: Unexpected error occurred.\n");
    }
}

static const char *get_enclave_absolute_path() {
    static char enclave_path[MAX_PATH] = {0};
    // Get the absolute path of the executable
    readlink("/proc/self/exe", enclave_path, sizeof(enclave_path));
    // Get the absolute path of the containing directory
    dirname(enclave_path);
    // Get the absolute path of the enclave
    strncat(enclave_path, "/../lib/", sizeof(enclave_path));
    strncat(enclave_path, ENCLAVE_FILENAME, sizeof(enclave_path));
    return (const char *)enclave_path;
}

/* Initialize the enclave:
 *   Step 1: try to retrieve the launch token saved by last transaction
 *   Step 2: call sgx_create_enclave to initialize an enclave instance
 *   Step 3: save the launch token if it is updated
 */
static int initialize_enclave(void) {
    char token_path[MAX_PATH] = {'\0'};
    sgx_launch_token_t token = {0};
    sgx_status_t ret = SGX_ERROR_UNEXPECTED;
    int updated = 0;

    /* Step 1: try to retrieve the launch token saved by last transaction
     *         if there is no token, then create a new one.
     */
    /* try to get the token saved in $HOME */
    const char *home_dir = getpwuid(getuid())->pw_dir;

    if (home_dir != NULL &&
            (strlen(home_dir) + strlen("/") + sizeof(TOKEN_FILENAME) + 1) <= MAX_PATH) {
        /* compose the token path */
        strncpy(token_path, home_dir, strlen(home_dir));
        strncat(token_path, "/", strlen("/"));
        strncat(token_path, TOKEN_FILENAME, sizeof(TOKEN_FILENAME) + 1);
    } else {
        /* if token path is too long or $HOME is NULL */
        strncpy(token_path, TOKEN_FILENAME, sizeof(TOKEN_FILENAME));
    }

    FILE *fp = fopen(token_path, "rb");
    if (fp == NULL && (fp = fopen(token_path, "wb")) == NULL) {
        printf("Warning: Failed to create/open the launch token file \"%s\".\n", token_path);
    }

    if (fp != NULL) {
        /* read the token from saved file */
        size_t read_num = fread(token, 1, sizeof(sgx_launch_token_t), fp);
        if (read_num != 0 && read_num != sizeof(sgx_launch_token_t)) {
            /* if token is invalid, clear the buffer */
            memset(&token, 0x0, sizeof(sgx_launch_token_t));
            printf("Warning: Invalid launch token read from \"%s\".\n", token_path);
        }
    }

    /* Step 2: call sgx_create_enclave to initialize an enclave instance */
    /* Debug Support: set 2nd parameter to 1 */
    const char *enclave_path = get_enclave_absolute_path();
    ret = sgx_create_enclave(enclave_path, SGX_DEBUG_FLAG, &token, &updated, &global_eid,
                             NULL);
    if (ret != SGX_SUCCESS) {
        print_error_message(ret);
        if (fp != NULL) { fclose(fp); }
        return -1;
    }

    /* Step 3: save the launch token if it is updated */
    if (updated == 0 || fp == NULL) {
        /* if the token is not updated, or file handler is invalid, do not perform saving */
        if (fp != NULL) { fclose(fp); }
        return 0;
    }

    /* reopen the file with write capablity */
    fp = freopen(token_path, "wb", fp);
    if (fp == NULL) { return 0; }
    size_t write_num = fwrite(token, 1, sizeof(sgx_launch_token_t), fp);
    if (write_num != sizeof(sgx_launch_token_t)) {
        printf("Warning: Failed to save launch token to \"%s\".\n", token_path);
    }
    fclose(fp);
    return 0;
}


// File stream for output buffer
static FILE *fp_output = NULL;

// ==========================================================================
//  OCalls
// ==========================================================================

void ocall_print(const char *str) {
    if (fp_output) {
        fprintf(fp_output, "%s", str);
    } else {
        fprintf(stdout, "%s", str);
    }
}

void ocall_eprint(const char *str) {
    fprintf(stderr, "%s", str);
}

int ocall_open_for_write(const char *path) {
    return open(path, O_WRONLY | O_CREAT | O_TRUNC, 00644);
}

int ocall_open_for_read(const char *path) {
    return open(path, O_RDONLY);
}

ssize_t ocall_read(int fd, void *buf, size_t size) {
    return read(fd, buf, size);
}

ssize_t ocall_write(int fd, const void *buf, size_t size) {
    return write(fd, buf, size);
}

int ocall_close(int fd) {
    return close(fd);
}

// ==========================================================================
//  Parsing program arguments
// ==========================================================================

static void print_help(void) {
    fprintf(stderr,
            "Error: invalid arguments\n"
            "\n"
            "Usage:\n"
            "\tprotect-integrity protect <ordinary_file>\n"
            "\tprotect-integrity show <protected_file> [<output_file>]\n"
            "\tprotect-integrity show-mac <protected_file> [<output_file>]\n");
}

#define CMD_ERROR       (-1)
#define CMD_PROTECT     0
#define CMD_SHOW        1
#define CMD_SHOW_MAC    2

static int parse_args(
    /* inputs */
    int argc,
    char *argv[],
    /* outputs */
    int *arg_command,
    char **arg_file_path,
    char **arg_output_path) {
    if (argc < 3 || argc > 4) { return -1; }

    if (strcmp(argv[1], "protect") == 0) {
        if (argc != 3) { return -1; }
        *arg_command = CMD_PROTECT;
    } else if (strcmp(argv[1], "show") == 0) {
        *arg_command = CMD_SHOW;
        if (argc == 4) {
            *arg_output_path = argv[3];
        }
    } else if (strcmp(argv[1], "show-mac") == 0) {
        *arg_command = CMD_SHOW_MAC;
        if (argc == 4) {
            *arg_output_path = argv[3];
        }
    } else {
        return -1;
    }

    *arg_file_path = argv[2];
    return 0;
}

// ==========================================================================
//  Main
// ==========================================================================

int SGX_CDECL main(int argc, char *argv[]) {
    /* Parse arguments */
    int arg_command = CMD_ERROR;
    char *arg_file_path = NULL;
    char *arg_output_path = NULL;
    if (parse_args(argc, argv, &arg_command, &arg_file_path, &arg_output_path) < 0) {
        print_help();
        return -1;
    }

    /* Initialize the enclave */
    if (initialize_enclave() < 0) {
        fprintf(stderr, "Error: enclave initialization failed\n");
        return -1;
    }

    /* Do the command */
    int ret = 0;
    switch (arg_command) {
        case CMD_PROTECT: {
            const char *input_path = arg_file_path;

            const char *output_ext = ".protected";
            size_t output_path_len = strlen(input_path) + strlen(output_ext) + 1;

            char *output_path = (char *) malloc(output_path_len);
            strncpy(output_path, input_path, output_path_len);
            strncat(output_path, output_ext, output_path_len);

            if (ecall_protect(global_eid, &ret, input_path, output_path)) {
                fprintf(stderr, "Error: ecall failed\n");
                ret = -1;
            }
            break;
        }
        case CMD_SHOW: {
            const char *input_path = arg_file_path;
            const char *output_path = arg_output_path;
            if (ecall_show(global_eid, &ret, input_path, output_path)) {
                fprintf(stderr, "Error: ecall failed\n");
                ret = -1;
            }
            break;
        }
        case CMD_SHOW_MAC: {
            const char *input_path = arg_file_path;
            const char *output_path = arg_output_path;
            if (output_path) {
                fp_output = fopen(output_path, "w");
                if (!fp_output) {
                    fprintf(stderr, "Error: failed to open %s for output \n", output_path);
                    ret = -1;
                    break;
                }
            }
            if (ecall_show_mac(global_eid, &ret, input_path)) {
                fprintf(stderr, "Error: ecall failed\n");
                ret = -1;
            }
            if (fp_output) {
                fclose(fp_output);
            }
            break;
        }
        default: {
            // This should never happen!
            abort();
        }
    }

    /* Destroy the enclave */
    sgx_destroy_enclave(global_eid);
    return ret;
}
