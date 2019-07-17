#include <sys/types.h>
#include <sys/uio.h>
#include <fcntl.h>
#include <unistd.h>
#include <string.h>
#include <stdio.h>

int main(int argc, const char* argv[]) {
    int fd, len;
    char read_buf[128] = {0};

    // read
    if ((fd = open("/host/hostfs/sample.txt", O_RDONLY)) < 0) {
        printf("ERROR: failed to open a file for read\n");
        return -1;
    }
    if ((len = read(fd, read_buf, sizeof(read_buf) - 1)) <= 0) {
        printf("ERROR: failed to read from the file\n");
        return -1;
    }
    close(fd);

    if (strcmp("HostFS works!", read_buf) != 0) {
        printf("ERROR: the message read from the file is not expected\n");
        return -1;
    }
    printf("Read file from hostfs successfully!\n");

    // write
    if ((fd = open("/host/hostfs/test_write.txt", O_WRONLY | O_CREAT)) < 0) {
        printf("ERROR: failed to open a file for write\n");
        return -1;
    }
    const char WRITE_STR[] = "Write to hostfs successfully!";
    if ((len = write(fd, WRITE_STR, sizeof(WRITE_STR))) <= 0) {
        printf("ERROR: failed to write to the file\n");
        return -1;
    }
    close(fd);

    printf("Write file to hostfs finished. Please check its content.\n");
    return 0;
}
