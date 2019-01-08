#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <unistd.h>
#include <string.h>
#include <stdio.h>

int main(int argc, const char* argv[]) {
    const char* file_name = "tmp.txt";
    int fd, flags, mode, len;
    const char* write_msg = "Hello World\n";
    char read_buf[128] = {0};

    flags = O_WRONLY | O_CREAT| O_TRUNC;
    mode = 00666;
    if ((fd = open(file_name, flags, mode)) < 0) {
        printf("ERROR: failed to open a file for write\n");
        return -1;
    }
    if ((len = write(fd, write_msg, strlen(write_msg))) <= 0) {
        printf("ERROR: failed to write to the file\n");
        return -1;
    }
    close(fd);

    flags = O_RDONLY;
    if ((fd = open(file_name, flags)) < 0) {
        printf("ERROR: failed to open a file for read\n");
        return -1;
    }
    if ((len = read(fd, read_buf, sizeof(read_buf) - 1)) <= 0) {
        printf("ERROR: failed to read from the file\n");
        return -1;
    }
    close(fd);

    if (strcmp(write_msg, read_buf) != 0) {
        printf("ERROR: the message read from the file is not as it was written\n");
        return -1;
    }

    printf("File write and read succesfully\n");
    return 0;
}
