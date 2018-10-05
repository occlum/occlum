#include "rusgx_stub.h"

static char success_str_buf[] = "Success!\n";
static unsigned long success_str_size = sizeof(success_str_buf);

static void print_ok(void) {
    __rusgx_write(1, success_str_buf, success_str_size);
}


static int test_write(const char* file_path) {
    int write_fd = __rusgx_open(file_path, O_WRONLY | O_CREAT | O_TRUNC, 0666);
    if (write_fd < 0) {
        return -1;
    }

    char write_buf[] = "Hello World!\n";
    size_t write_len = sizeof(write_buf);
    if (__rusgx_write(write_fd, write_buf, write_len) != write_len) {
        return -2;
    }

    if (__rusgx_close(write_fd) < 0) {
        return -3;
    }

    return 0;
}

static int test_read(const char* file_path) {
    int read_fd = __rusgx_open(file_path, O_RDONLY, 0);
    if (read_fd < 0) {
        return -1;
    }

    char read_buf[256] = { 0 };
    size_t read_len;
    if ((read_len = __rusgx_read(read_fd, read_buf, 256)) < 0 ) {
        return -2;
    }

    __rusgx_write(1, read_buf, read_len);

    if (__rusgx_close(read_fd) < 0) {
        return -3;
    }

    return 0;
}

void _start(void) {
    int ret = 0;
    const char* file_path = "tmp.txt.protected";

    if ((ret = test_write(file_path)) < 0) {
        goto on_exit;
    }

    if ((ret = test_read(file_path)) < 0) {
        goto on_exit;
    }

    print_ok();
on_exit:
    __rusgx_exit(ret);
}
