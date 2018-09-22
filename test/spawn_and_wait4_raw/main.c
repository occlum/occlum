#include "rusgx_stub.h"

static char success_str_buf[] = "A child process starts and exits!\n";
static unsigned long success_str_size = sizeof(success_str_buf);

static void print_ok(void) {
    __rusgx_write(1, success_str_buf, success_str_size);
}

void _start(void) {
    int ret = 0;
    int pid = 0;

    ret = __rusgx_spawn(&pid, "hello_world_raw/bin.encrypted", NULL, NULL);
    if (ret < 0) { __rusgx_exit(0); }

    int status;
    ret = __rusgx_wait4(pid, &status, 0);
    if (ret < 0) { __rusgx_exit(0); }

    print_ok();

    __rusgx_exit(0);
}
