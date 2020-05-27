#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <unistd.h>
#include <string.h>
#include <stdio.h>
#include <errno.h>

int main(int argc, const char *argv[]) {
    const char *file_name = "/root/test_filesystem_link.txt";
    const char *link_name = "/root/link.txt";
    const char *write_msg = "Hello World\n";
    char read_buf[128] = {0};
    int ret;

    // create a file and write message
    int flags = O_WRONLY | O_CREAT | O_TRUNC;
    int mode = 00666;
    int fd = open(file_name, flags, mode);
    if (fd < 0) {
        printf("ERROR: failed to open a file for write\n");
        return -1;
    }
    int len = write(fd, write_msg, strlen(write_msg));
    if (len <= 0) {
        printf("ERROR: failed to write to the file\n");
        return -1;
    }
    close(fd);

    // link
    ret = link(file_name, link_name);
    if (ret < 0) {
        printf("ERROR: failed to link the file\n");
        return -1;
    }

    // read the link file
    fd = open(link_name, O_RDONLY, 00666);
    if (fd < 0) {
        printf("ERROR: failed to open the file for read\n");
        return -1;
    }
    len = read(fd, read_buf, sizeof(read_buf));
    if (len != strlen(write_msg)) {
        printf("ERROR: failed to read to the file\n");
        return -1;
    }
    ret = strcmp(write_msg, read_buf);
    if (ret != 0) {
        printf("ERROR: the message read from the file is not as it was written\n");
        return -1;
    }

    // unlink
    ret = unlink(link_name);
    if (ret < 0) {
        printf("ERROR: failed to link the file\n");
        return -1;
    }

    // stat
    struct stat stat_buf;
    ret = stat(link_name, &stat_buf);
    if (!(ret < 0 && errno == ENOENT)) {
        printf("ERROR: stat on \"%s\" should return ENOENT", link_name);
        return -1;
    }

    // rename
    ret = rename(file_name, link_name);
    if (ret < 0) {
        printf("ERROR: failed to rename the file");
        return -1;
    }

    // stat
    ret = stat(file_name, &stat_buf);
    if (!(ret < 0 && errno == ENOENT)) {
        printf("ERROR: stat on \"%s\" should return ENOENT", file_name);
        return -1;
    }
    ret = stat(link_name, &stat_buf);
    if (ret < 0) {
        printf("ERROR: failed to stat the file");
        return -1;
    }

    printf("link, unlink, rename test successful\n");
    return 0;
}
