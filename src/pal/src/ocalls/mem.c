#include <stdlib.h>
#include <sys/mman.h>
#include "ocalls.h"

void *occlum_ocall_posix_memalign(size_t alignment, size_t size) {
    void *ptr = NULL;
    int ret = posix_memalign(&ptr, alignment, size);
    if (ret == 0) {
        return ptr;
    }

    // Handle errors
    switch (ret) {
        case ENOMEM:
            PAL_ERROR("Out of memory on the untrusted side");
            break;
        case EINVAL:
            PAL_ERROR("Invalid arguments given to occlum_ocall_posix_memalign");
            break;
        default:
            PAL_ERROR("Unexpected error in occlum_ocall_posix_memalign");
    }
    return NULL;
}

void occlum_ocall_free(void *ptr) {
    free(ptr);
}

int occlum_ocall_mprotect(void *addr, size_t len, int prot) {
    return mprotect(addr, len, prot);
}
