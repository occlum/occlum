#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <libgen.h>
#include <pwd.h>
#include <sched.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/time.h>
#include <unistd.h>
#include <sys/socket.h>

#include <sgx_eid.h>
#include <sgx_error.h>
#include <sgx_urts.h>

#include "pal_enclave.h"
#include "pal_error.h"
#include "pal_log.h"
#include "base64.h"

#define MAX_PATH            FILENAME_MAX
#define TOKEN_FILENAME      "enclave.token"
#define ENCLAVE_FILENAME    "libocclum-libos.signed.so"

static sgx_enclave_id_t global_eid = SGX_INVALID_ENCLAVE_ID;

/* Get enclave debug flag according to env "OCCLUM_RELEASE_ENCLAVE" */
static int get_enclave_debug_flag() {
    const char *release_enclave_val = getenv("OCCLUM_RELEASE_ENCLAVE");
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

/* Get enable kss flag according to env "OCCLUM_ENABLE_KSS" */
static int get_enable_kss_flag() {
    const char *enable_kss_val = getenv("OCCLUM_ENABLE_KSS");
    if (enable_kss_val) {
        if (!strcmp(enable_kss_val, "1") ||
                !strcasecmp(enable_kss_val, "y") ||
                !strcasecmp(enable_kss_val, "yes") ||
                !strcasecmp(enable_kss_val, "true")) {
            return 1;
        }
    }
    return 0;
}

static const char *get_enclave_absolute_path(const char *instance_dir) {
    static char enclave_path[MAX_PATH + 1] = {0};
    strncat(enclave_path, instance_dir, MAX_PATH);
    strncat(enclave_path, "/build/lib/", MAX_PATH);
    strncat(enclave_path, ENCLAVE_FILENAME, MAX_PATH);
    return (const char *)enclave_path;
}

/* Initialize the enclave:
 *   Step 1: try to retrieve the launch token saved by last transaction
 *   Step 2: call sgx_create_enclave to initialize an enclave instance
 *   Step 3: save the launch token if it is updated
 */
int pal_init_enclave(const char *instance_dir) {
    char token_path[MAX_PATH] = {'\0'};
    sgx_launch_token_t token = {0};
    sgx_status_t ret = SGX_ERROR_UNEXPECTED;
    int updated = 0;

    /* Step 1: try to retrieve the launch token saved by last transaction
     *         if there is no token, then create a new one.
     */
    /* try to get the token saved in $HOME */
    const char *home_dir = NULL;
    struct passwd *pw = getpwuid(getuid());
    if (pw != NULL) {
        home_dir = pw->pw_dir;
    }

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
        PAL_WARN("Warning: Failed to create/open the launch token file \"%s\".\n", token_path);
    }

    if (fp != NULL) {
        /* read the token from saved file */
        size_t read_num = fread(token, 1, sizeof(sgx_launch_token_t), fp);
        if (read_num != 0 && read_num != sizeof(sgx_launch_token_t)) {
            /* if token is invalid, clear the buffer */
            memset(&token, 0x0, sizeof(sgx_launch_token_t));
            PAL_WARN("Invalid launch token read from \"%s\".\n", token_path);
        }
    }

    /* Step 2: call sgx_create_enclave to initialize an enclave instance */
    /* Debug Support: set 2nd parameter to 1 */
    const char *enclave_path = get_enclave_absolute_path(instance_dir);
    int sgx_debug_flag = get_enclave_debug_flag();
    int sgx_enable_kss = get_enable_kss_flag();

    /* If enable kss, use sgx_create_enclave_ex to create enclave */
    if (sgx_enable_kss) {
        sgx_kss_config_t kss_config = { 0 };
        const void *enclave_ex_p[32] = { 0 };
        const char *sgx_conf_id = getenv("OCCLUM_CONF_ID_BASE64");
        const char *sgx_conf_svn = getenv("OCCLUM_CONF_SVN");

        if (sgx_conf_id) {
            base64_decode(sgx_conf_id, kss_config.config_id, SGX_CONFIGID_SIZE);
        }

        if (sgx_conf_svn) {
            unsigned long svn_val = strtoul(sgx_conf_svn, NULL, 0);
            /* CONFIG SVN is 16 bits long */
            if (svn_val > 0xFFFF) {
                PAL_WARN("Invalid CONFIG SVN value: 0x%lx\n", svn_val);
            } else {
                kss_config.config_svn = svn_val;
            }
        }

        enclave_ex_p[SGX_CREATE_ENCLAVE_EX_KSS_BIT_IDX] = (const void *)&kss_config;
        ret = sgx_create_enclave_ex(enclave_path, sgx_debug_flag, &token, &updated, &global_eid,
                                    NULL, SGX_CREATE_ENCLAVE_EX_KSS, enclave_ex_p);
    } else {
        ret = sgx_create_enclave(enclave_path, sgx_debug_flag, &token, &updated, &global_eid,
                                 NULL);
    }
    if (ret != SGX_SUCCESS) {
        const char *sgx_err_msg = pal_get_sgx_error_msg(ret);
        PAL_ERROR("Failed to create enclave with error code 0x%x: %s", ret, sgx_err_msg);
        if (fp != NULL) { fclose(fp); }
        return -1;
    }

    /* Step 3: save the launch token if it is updated */
    if (updated == 0 || fp == NULL) {
        /* if the token is not updated, or file handler is invalid, do not perform saving */
        if (fp != NULL) { fclose(fp); }
        return 0;
    }

    /* reopen the file with write capability */
    fp = freopen(token_path, "wb", fp);
    if (fp == NULL) { return 0; }
    size_t write_num = fwrite(&token, 1, sizeof(sgx_launch_token_t), fp);
    if (write_num != sizeof(sgx_launch_token_t)) {
        PAL_WARN("Warning: Failed to save launch token to \"%s\".\n", token_path);
    }
    fclose(fp);
    return 0;
}

int pal_destroy_enclave(void) {
    sgx_destroy_enclave(global_eid);
    global_eid = SGX_INVALID_ENCLAVE_ID;
    return 0;
}

sgx_enclave_id_t pal_get_enclave_id(void) {
    return global_eid;
}
