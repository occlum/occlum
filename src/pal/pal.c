#include "Enclave_u.h"

#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <libgen.h>
#include <pwd.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/time.h>
#include <unistd.h>

#include <sgx_eid.h>
#include <sgx_error.h>
#include <sgx_urts.h>

#include "task.h"

#define MAX_PATH            FILENAME_MAX
#define TOKEN_FILENAME      "enclave.token"
#define ENCLAVE_FILENAME    "libocclum.signed.so"

// ==========================================================================
//  Enclave Initialization
// ==========================================================================

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
    {
        SGX_ERROR_SERVICE_INVALID_PRIVILEGE,
        "Enclave has no privilege to get run in the release mode.",
        "Please rebuild the Occlum enclave with a legal signing key "
        "(e.g., occlum build --sign-key <key_path>), "
        "to get a legal signing key, please contact Intel."
    },
};

/* Check error conditions for loading enclave */
static void print_error_message(sgx_status_t ret) {
    size_t idx = 0;
    size_t ttl = sizeof sgx_errlist/sizeof sgx_errlist[0];

    for (idx = 0; idx < ttl; idx++) {
        if(ret == sgx_errlist[idx].err) {
            printf("Error: %s\n", sgx_errlist[idx].msg);
            if(NULL != sgx_errlist[idx].sug)
                printf("Info: %s\n", sgx_errlist[idx].sug);
            break;
        }
    }

    if (idx == ttl)
        printf("Error: Unexpected error occurred.\n");
}

static const char* get_enclave_absolute_path() {
    static char enclave_path[MAX_PATH] = {0};
    // Get the absolute path of the executable
    readlink("/proc/self/exe", enclave_path, sizeof(enclave_path));
    // Get the absolute path of the containing directory
    dirname(enclave_path);
    // Get the absolute path of the enclave
    strncat(enclave_path, "/../lib/", sizeof(enclave_path));
    strncat(enclave_path, ENCLAVE_FILENAME, sizeof(enclave_path));
    return (const char*)enclave_path;
}

/* Get enclave debug flag according to env "OCCLUM_RELEASE_ENCLAVE" */
static int get_enclave_debug_flag() {
    const char* release_enclave_val = getenv("OCCLUM_RELEASE_ENCLAVE");
    if (release_enclave_val) {
        if (!strcmp(release_enclave_val, "1") ||
            !strcasecmp(release_enclave_val, "y") ||
            !strcasecmp(release_enclave_val, "yes") ||
            !strcasecmp(release_enclave_val, "true")) {
            return 0;
        }
    }
    return 1;
}

/* Initialize the enclave:
 *   Step 1: try to retrieve the launch token saved by last transaction
 *   Step 2: call sgx_create_enclave to initialize an enclave instance
 *   Step 3: save the launch token if it is updated
 */
static int initialize_enclave()
{
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
        (strlen(home_dir)+strlen("/")+sizeof(TOKEN_FILENAME)+1) <= MAX_PATH) {
        /* compose the token path */
        strncpy(token_path, home_dir, strlen(home_dir));
        strncat(token_path, "/", strlen("/"));
        strncat(token_path, TOKEN_FILENAME, sizeof(TOKEN_FILENAME)+1);
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
    const char* enclave_path = get_enclave_absolute_path();
    int sgx_debug_flag = get_enclave_debug_flag();
    ret = sgx_create_enclave(enclave_path, sgx_debug_flag, &token, &updated, &global_eid, NULL);
    if (ret != SGX_SUCCESS) {
        print_error_message(ret);
        if (fp != NULL) fclose(fp);
        return -1;
    }

    /* Step 3: save the launch token if it is updated */
    if (updated == 0 || fp == NULL) {
        /* if the token is not updated, or file handler is invalid, do not perform saving */
        if (fp != NULL) fclose(fp);
        return 0;
    }

    /* reopen the file with write capablity */
    fp = freopen(token_path, "wb", fp);
    if (fp == NULL) return 0;
    size_t write_num = fwrite(token, 1, sizeof(sgx_launch_token_t), fp);
    if (write_num != sizeof(sgx_launch_token_t))
        printf("Warning: Failed to save launch token to \"%s\".\n", token_path);
    fclose(fp);
    return 0;
}

// ==========================================================================
//  OCalls
// ==========================================================================

void ocall_print_string(const char* msg) {
    printf("%s", msg);
}

int ocall_run_new_task(void) {
    int ret = run_new_task(global_eid);
    return ret;
}

void ocall_gettimeofday(long* seconds, long* microseconds) {
    struct timeval tv;
    gettimeofday(&tv, NULL);
    *seconds = tv.tv_sec;
    *microseconds = tv.tv_usec;
}

void ocall_clock_gettime(int clockid, time_t* sec, long* ns) {
    struct timespec ts;
    clock_gettime(clockid, &ts);
    *sec = ts.tv_sec;
    *ns = ts.tv_nsec;
}

int ocall_sched_getaffinity(int* error, int pid, size_t cpusize, unsigned char* buf) {
    int ret = syscall(__NR_sched_getaffinity, pid, cpusize, buf);
    if (error) {
        *error = (ret == -1) ? errno : 0;
    }
    return ret;
}

int ocall_sched_setaffinity(int* error, int pid, size_t cpusize, const unsigned char* buf) {
    int ret = syscall(__NR_sched_setaffinity, pid, cpusize, buf);
    if (error) {
        *error = (ret == -1) ? errno : 0;
    }
    return ret;
}

void ocall_sync(void) {
    sync();
}

// ==========================================================================
//  Main
// ==========================================================================

/* Application entry */
int SGX_CDECL main(int argc, const char *argv[])
{
    struct timeval startup, libosready, appdie;

    gettimeofday(&startup, NULL);
    sgx_status_t sgx_ret = SGX_SUCCESS;
    int status = 0;

    if (argc < 2) {
        printf("ERROR: at least one argument must be provided\n\n");
        printf("Usage: pal <executable> <arg1> <arg2>...\n");
        return -1;
    }
    const char* executable_path = argv[1];

    /* Initialize the enclave */
    if(initialize_enclave() < 0){
        printf("Enter a character before exit ...\n");
        getchar();
        return -1;
    }

    // First ecall do a lot initializations.
    // Count it as startup time.
    dummy_ecall(global_eid, &status);

    gettimeofday(&libosready, NULL);

    sgx_ret = libos_boot(global_eid, &status, executable_path, &argv[2]);
    if(sgx_ret != SGX_SUCCESS) {
        print_error_message(sgx_ret);
        return status;
    }

    status = wait_all_tasks();

    gettimeofday(&appdie, NULL);

    uint64_t libos_startup_time, app_runtime;
    libos_startup_time = (libosready.tv_sec - startup.tv_sec) * 1000000 + (libosready.tv_usec - startup.tv_usec);
    app_runtime = (appdie.tv_sec - libosready.tv_sec) * 1000000 + (appdie.tv_usec - libosready.tv_usec);
    printf("LibOS startup time: %lu microseconds\n", libos_startup_time);
    printf("Apps running time: %lu microseconds\n", app_runtime);

    /* Destroy the enclave */
    sgx_destroy_enclave(global_eid);

    return status;
}
