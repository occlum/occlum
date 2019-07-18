#include <stdio.h>
#include <stdint.h>

static inline uint64_t native_rdtsc() {
    uint32_t hi, lo;
    asm volatile("rdtsc" : "=a"(lo), "=d"(hi));
    return (( (uint64_t)lo)|( ((uint64_t)hi)<<32 ));
}

int main(int argc, char **argv)
{
    /* Gets rdtsc information and tests the SGX support of the rdtsc */
    uint64_t r;

    r = native_rdtsc();
    printf("rdtsc: %lu\n", r);

    return 0;
}
