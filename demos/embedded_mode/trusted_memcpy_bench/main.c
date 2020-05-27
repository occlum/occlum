#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/time.h>

//============================================================================
// Help message
//============================================================================

#define HELP_MSG                                                            \
    "Usage: %s <buf_ptr> <buf_size> <total_bytes>\n"                        \
    "\n"                                                                    \
    "Arguments:\n"                                                          \
    "    <buf_ptr>      The pointer to an untrusted buffer outside the enclave\n" \
    "    <buf_size>     The size of the untrusted buffer\n"                 \
    "    <total_bytes>  The total number of bytes to copy from the buffer into the enclave\n"

static void print_help_msg(const char *prog_name) {
    fprintf(stderr, HELP_MSG, prog_name);
}

//============================================================================
// Data consumption
//============================================================================

#define MIN(x, y)       ((x) <= (y) ? (x) : (y))

static int copy_into_enclave(const char *src_buf, size_t buf_size, size_t total_bytes) {
    char *dst_buf = malloc(buf_size);
    if (dst_buf == NULL) {
        fprintf(stderr, "ERROR: out of memory");
        return -1;
    }

    while (total_bytes > 0) {
        size_t copy_bytes = MIN(buf_size, total_bytes);
        memcpy(dst_buf, src_buf, copy_bytes);
        total_bytes -= copy_bytes;
    }

    free(dst_buf);
    return 0;
}

//============================================================================
// Main
//============================================================================

int main(int argc, char *argv[]) {
    // Parse arguments
    const char *prog_name = argv[0];
    if (argc < 4) {
        print_help_msg(prog_name);
        return EXIT_FAILURE;
    }
    const char *buf_ptr = (const char *) strtoul(argv[1], NULL, 10);
    size_t buf_size = (size_t) strtoul(argv[2], NULL, 10);
    size_t total_bytes = (size_t) strtoul(argv[3], NULL, 10);
    if (buf_ptr == NULL || buf_size == 0 || total_bytes == 0) {
        print_help_msg(prog_name);
        return EXIT_FAILURE;
    }

    // Benchmark memcpy from outside the enclave to inside the enclave
    printf("Start copying data from the given buffer (ptr = %p, len = %lu) for a total of %lu bytes...\n",
           buf_ptr, buf_size, total_bytes);

    // Time begin
    struct timeval time_begin, time_end;
    gettimeofday(&time_begin, NULL);
    // Do memcpy for a total of `total_bytes` bytes
    int ret = copy_into_enclave(buf_ptr, buf_size, total_bytes);
    if (ret < 0) {
        return EXIT_FAILURE;
    }
    // Time end
    gettimeofday(&time_end, NULL);
    printf("Done.\n");

    // Calculate the throughput
    unsigned long elapsed_us = (time_end.tv_sec - time_begin.tv_sec) * 1000000
                               + (time_end.tv_usec - time_begin.tv_usec);
    if (elapsed_us == 0) {
        fprintf(stderr, "ERROR: elapsed time (in us) cannot be zero");
        print_help_msg(prog_name);
        return EXIT_FAILURE;
    }
    printf("Cross-enclave memcpy throughput = %lu MB/s\n", total_bytes / elapsed_us);

    return EXIT_SUCCESS;
}
