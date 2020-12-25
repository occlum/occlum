#include <sys/utsname.h>
#include <stdlib.h>
#include <stdio.h>

#define MAX_SIZE    (1*4*1024)
#define MIN_SIZE    8

void test_uname() {
    struct utsname name;

    printf("Testing uname...\n");
    uname(&name);
    printf("sysname = %s\n", (const char *)&name.sysname);
    printf("nodename = %s\n", (const char *)&name.nodename);
    printf("release = %s\n", (const char *)&name.release);
    printf("version = %s\n", (const char *)&name.version);
    printf("machine = %s\n", (const char *)&name.machine);
    printf("domainname = %s\n", (const char *)&name.__domainname);
}

int test_malloc_free() {
    printf("Testing malloc and free...\n");

    for (size_t buf_size = MIN_SIZE; buf_size <= MAX_SIZE; buf_size *= 4) {
        printf("buf_size = %lu\n", buf_size);
        void *buf = malloc(buf_size);
        if (buf == NULL) {
            printf("ERROR: failed to malloc for a buffer of %lu size\n", buf_size);
            return -1;
        }
        free(buf);
    }

    printf("Done.\n");

    return 0;
}

int main() {
    test_uname();

    int ret = test_malloc_free();

    return ret;
}
