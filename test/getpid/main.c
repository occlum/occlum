#include "rusgx_stub.h"

void _start(void) {
    int pid = __rusgx_getpid();
    (void)pid;

    __rusgx_exit(0);
}
