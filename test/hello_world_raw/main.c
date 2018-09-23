#include "rusgx_stub.h"

char str_buf[] = "Hello World!\n";
unsigned long str_size = sizeof(str_buf);

void _start(void) {
    //__rusgx_write(1, str_buf, str_size);
    __rusgx_exit(0);
}
