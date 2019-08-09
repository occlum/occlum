#include <stddef.h>

// The total size of the memory available to user programs
// Should be provided by Makefile
#ifndef OCCLUM_BUILTIN_VM_USER_SPACE_SIZE
#define OCCLUM_BUILTIN_VM_USER_SPACE_SIZE      (128*1024*1024)
#endif

static char __preallocated_memory[OCCLUM_BUILTIN_VM_USER_SPACE_SIZE]
    __attribute__ ((
        section(".exectuable_data,\"awx\",@nobits#"),
        aligned(4096))) = {0};

void vm_get_preallocated_user_space_memory(void** paddr, size_t* psize) {
    *paddr = __preallocated_memory;
    *psize = sizeof(__preallocated_memory);
}
