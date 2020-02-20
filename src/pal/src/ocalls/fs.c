#include <unistd.h>
#include "ocalls.h"
#include <sys/eventfd.h>

void occlum_ocall_sync(void) {
    sync();
}

int occlum_ocall_eventfd(unsigned int initval, int flags) {
    return eventfd(initval, flags);
}
