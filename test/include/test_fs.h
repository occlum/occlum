#ifndef __TEST_FS_H
#define __TEST_FS_H

#include <linux/limits.h>
#include <stdio.h>
#include <string.h>
#include <libgen.h>
#include <unistd.h>
#include "test.h"

int fs_split_path(const char *path, char *dir_buf, char **dir_name, char *base_buf,
                  char **base_name) {
    size_t ret;

    if (path == NULL) {
        THROW_ERROR("input path is NULL");
    }
    if (dir_buf != NULL) {
        if (dir_name == NULL) {
            THROW_ERROR("dir_name is NULL");
        }
        ret = snprintf(dir_buf, PATH_MAX, "%s", path);
        if (ret >= PATH_MAX || ret < 0) {
            THROW_ERROR("failed to copy file path to the dir buffer");
        }
        *dir_name = dirname(dir_buf);
    }
    if (base_buf != NULL) {
        if (base_name == NULL) {
            THROW_ERROR("base_name is NULL");
        }
        ret = snprintf(base_buf, PATH_MAX, "%s", path);
        if (ret >= PATH_MAX || ret < 0) {
            THROW_ERROR("failed to copy file path to the base buffer");
        }
        *base_name = basename(base_buf);
    }
    return 0;
}

int fs_check_file_content(const char *path, const char *msg) {
    char read_buf[128] = { 0 };

    int fd = open(path, O_RDONLY);
    if (fd < 0) {
        THROW_ERROR("failed to open file");
    }
    size_t len = read(fd, read_buf, sizeof(read_buf));
    if (len != strlen(msg)) {
        THROW_ERROR("failed to read the msg from file");
    }
    if (strcmp(msg, read_buf) != 0) {
        THROW_ERROR("the message read from the file is not expected");
    }
    close(fd);
    return 0;
}

#endif /* __TEST_FS_H */
