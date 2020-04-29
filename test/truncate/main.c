#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <unistd.h>
#include <string.h>
#include <stdio.h>

int main(int argc, const char* argv[]) {
    const char* FILE_NAME = "root/test_filesystem_truncate.txt";
    const int TRUNC_LEN = 256;
    const int TRUNC_LEN1 = 128;
    const int MODE_MASK = 0777;

    int ret;

    int flags = O_WRONLY | O_CREAT| O_TRUNC;
    int mode = 00666;
    int fd = open(FILE_NAME, flags, mode);
    if (fd < 0) {
        printf("failed to open a file for write\n");
        return fd;
    }

    if (access(FILE_NAME, F_OK) < 0) {
        printf("cannot access the new file\n");
        return -1;
    }

    ret = ftruncate(fd, TRUNC_LEN);
    if (ret < 0) {
        printf("failed to ftruncate the file\n");
        return ret;
    }

    struct stat stat_buf;
    ret = fstat(fd, &stat_buf);
    if (ret < 0) {
        printf("failed to fstat the file\n");
        return ret;
    }

    int file_size = stat_buf.st_size;
    if (file_size != TRUNC_LEN) {
        printf("Incorrect file size %d. Expected %d\n", file_size, TRUNC_LEN);
        return -1;
    }
    int file_mode = stat_buf.st_mode & MODE_MASK;
    if (file_mode != mode) {
        printf("Incorrect file mode %o. Expected %o\n", file_mode, mode);
        return -1;
    }
    int file_type = stat_buf.st_mode & S_IFMT;
    if (file_type != S_IFREG) {
        printf("Incorrect file type %o. Expected %o\n", file_type, S_IFREG);
        return -1;
    }

    close(fd);

    ret = truncate(FILE_NAME, TRUNC_LEN1);
    if (ret < 0) {
        printf("failed to truncate the file\n");
        return ret;
    }

    ret = stat(FILE_NAME, &stat_buf);
    if (ret < 0) {
        printf("failed to stat the file\n");
        return ret;
    }

    file_size = stat_buf.st_size;
    if (file_size != TRUNC_LEN1) {
        printf("Incorrect file size %d. Expected %d\n", file_size, TRUNC_LEN1);
        return -1;
    }

    printf("(f)truncate, (f)stat test successful\n");
    return 0;
}
