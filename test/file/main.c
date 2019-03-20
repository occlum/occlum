#include <sys/types.h>
#include <sys/uio.h>
#include <fcntl.h>
#include <unistd.h>
#include <string.h>
#include <stdio.h>

int main(int argc, const char* argv[]) {
    const char* file_name = "tmp.txt";
    int fd, flags, mode, len;
    off_t offset;
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


    flags = O_RDWR;
    if ((fd = open(file_name, flags)) < 0) {
        printf("ERROR: failed to open a file for read and write\n");
        return -1;
    }

    const char* iov_msg[2] = {"hello ", "world!"};
    struct iovec iov[2];
    for(int i=0; i<2; ++i) {
        iov[i].iov_base = (void*)iov_msg[i];
        iov[i].iov_len = strlen(iov_msg[i]);
    }
    if ((len = writev(fd, iov, 2)) != 12) {
        printf("ERROR: failed to write vectors to the file\n");
        return -1;
    }

    if ((offset = lseek(fd, 0, SEEK_SET)) != 0) {
        printf("ERROR: failed to lseek the file\n");
        return -1;
    }

    iov[0].iov_base = read_buf;
    iov[0].iov_len = 3;
    iov[1].iov_base = read_buf + 5;
    iov[1].iov_len = 20;
    if ((len = readv(fd, iov, 2)) != 12) {
        printf("ERROR: failed to read vectors from the file\n");
        return -1;
    }

    if (memcmp(read_buf, "hel", 3) != 0
        || memcmp(read_buf + 5, "lo world!", 9) != 0) {
        printf("ERROR: the message read from the file is not as it was written\n");
        return -1;
    }
    close(fd);

    printf("File write and read successfully\n");
    return 0;
}
