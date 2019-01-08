#include <unistd.h>
#include <stdio.h>

#define MAX_BUF_SIZE        (1 * 1024 * 1024)

#define MIN(x, y)           ((x) <= (y) ? (x) : (y))

int main(int argc, const char* argv[]) {
    // Get the total number of bytes to read
    size_t remain_bytes = 0;
    while (read(0, &remain_bytes, sizeof(remain_bytes)) != sizeof(remain_bytes));

    // Get the size of buffer to use
    size_t buf_size = 0;
    while (read(0, &buf_size, sizeof(buf_size)) != sizeof(buf_size));
    if (buf_size > MAX_BUF_SIZE) {
        printf("ERROR: the required buffer size (%lu) is tool large\n", buf_size);
        return -1;
    }

    // Read a specified amount of data in a buffer of specified size
    char buf[MAX_BUF_SIZE];
    while (remain_bytes > 0) {
        size_t len = MIN(buf_size, remain_bytes);
        if ((len = read(0, &buf, len)) < 0) {
            printf("ERROR: failed to write to pipe\n");
            return -1;
        }
        remain_bytes -= len;
    }

    return 0;
}
