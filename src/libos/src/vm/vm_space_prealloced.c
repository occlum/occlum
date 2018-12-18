#include <stddef.h>

#define DATA_SPACE_SIZE (16*1024*1024)

static char __prealloced_data_space[DATA_SPACE_SIZE]
    __attribute__ ((
        section(".exectuable_data,\"awx\",@nobits#"),
        aligned(4096))) = {0};

void vm_get_prealloced_data_space(void** paddr, size_t* psize) {
    *paddr = __prealloced_data_space;
    *psize = DATA_SPACE_SIZE;
}
