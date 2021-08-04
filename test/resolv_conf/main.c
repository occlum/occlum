#include <stdio.h>
#include <stdlib.h>
#include "test.h"

// ============================================================================
// Helper functions
// ============================================================================
char *read_resolv_conf(void) {
    FILE *fp = fopen("/etc/resolv.conf", "rb");
    fseek(fp, 0, SEEK_END);
    long fsize = ftell(fp);
    fseek(fp, 0, SEEK_SET);
    char *resolv_conf_buffer = malloc(fsize + 1);
    if (resolv_conf_buffer == NULL) {
        printf("ERROR: Failed to malloc for /etc/resolv.conf buffer\n");
        return NULL;
    }
    size_t len = fread(resolv_conf_buffer, 1, fsize, fp);
    if (len != fsize) {
        printf("ERROR: failed to fread correct len\n");
        fclose(fp);
        return NULL;
    }
    fclose(fp);
    return resolv_conf_buffer;
}

// ============================================================================
// Test cases for resolv.conf file
// ============================================================================

int test_resolv_conf() {
    char *buffer = read_resolv_conf();
    if (buffer == NULL) {
        THROW_ERROR("failed to read resolv.conf");
    }
    printf("%s", buffer);
    free(buffer);
    buffer = NULL;
    return 0;
}

static test_case_t test_cases[] = {
    TEST_CASE(test_resolv_conf),
};

// ============================================================================
// Test suite main
// ============================================================================

int main() {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
