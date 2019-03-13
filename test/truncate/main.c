#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <unistd.h>
#include <string.h>
#include <stdio.h>

int main(int argc, const char* argv[]) {
    const char* file_name = "tmp.txt";
    const int TRUNC_LEN = 256;
    const int MODE_MASK = 0777;

    int ret;

    int flags = O_WRONLY | O_CREAT| O_TRUNC;
    int mode = 00666;
    int fd = open(file_name, flags, mode);
    if (fd < 0) {
        printf("failed to open a file for write\n");
        return fd;
    }

    ret = ftruncate(fd, TRUNC_LEN);
    if (ret < 0) {
        printf("failed to truncate the file\n");
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

    printf("Truncate & fstat test succesful\n");
    return 0;
}
