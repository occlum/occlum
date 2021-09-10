#ifndef __TEST_FS_H
#define __TEST_FS_H

#include <linux/limits.h>
#include <stdio.h>
#include <string.h>
#include <libgen.h>
#include <unistd.h>
#include <dirent.h>
#include <stdbool.h>
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
    char read_buf[PATH_MAX] = { 0 };

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

int fill_file_with_repeated_bytes(int fd, size_t len, int byte_val) {
    char buf[1024 * 4];
    memset(buf, byte_val, sizeof(buf));

    size_t remain_bytes = len;
    while (remain_bytes > 0) {
        int to_write_bytes = MIN(sizeof(buf), remain_bytes);
        int written_bytes = write(fd, buf, to_write_bytes);
        if (written_bytes != to_write_bytes) {
            THROW_ERROR("file write failed");
        }
        remain_bytes -= written_bytes;
    }

    return 0;
}

int check_file_with_repeated_bytes(int fd, size_t len, int expected_byte_val) {
    int remain = len;
    char read_buf[512];
    while (remain > 0) {
        int read_nbytes = read(fd, read_buf, sizeof(read_buf));
        if (read_nbytes < 0) {
            THROW_ERROR("I/O error");
        }
        size_t check_nbytes = remain < read_nbytes ? remain : read_nbytes;
        remain -= read_nbytes;
        if (read_nbytes == 0 && remain > 0) {
            THROW_ERROR("Not enough data in the file");
        }
        if (check_bytes_in_buf(read_buf, check_nbytes, expected_byte_val) < 0) {
            THROW_ERROR("Incorrect data");
        }
    }
    return 0;
}

bool check_dir_entries(char entries[][NAME_MAX], int entry_cnt,
                       char expected_entries[][NAME_MAX], int expected_cnt) {
    for (int i = 0; i < expected_cnt; i++) {
        bool found = false;
        for (int j = 0; j < entry_cnt; j++) {
            if (strncmp(expected_entries[i], entries[j], strlen(expected_entries[i])) == 0) {
                found = true;
                break;
            }
        }
        if (!found) {
            printf("can't find: %s\n", expected_entries[i]);
            return false;
        }
    }
    return true;
}

int check_readdir_with_expected_entries(const char *dir_path,
                                        char expected_entries[][NAME_MAX],
                                        int expected_cnt) {
    struct dirent *dp;
    DIR *dirp;
    char entries[128][NAME_MAX] = { 0 };

    dirp = opendir(dir_path);
    if (dirp == NULL) {
        THROW_ERROR("failed to open directory");
    }

    int entry_cnt = 0;
    while (1) {
        errno = 0;
        dp = readdir(dirp);
        if (dp == NULL) {
            if (errno != 0) {
                THROW_ERROR("failed to call readdir");
            }
            break;
        }
        memcpy(entries[entry_cnt], dp->d_name, strlen(dp->d_name));
        ++entry_cnt;
    }

    if (!check_dir_entries(entries, entry_cnt, expected_entries, expected_cnt)) {
        THROW_ERROR("failed to check the result of readdir");
    }

    closedir(dirp);
    return 0;
}

#endif /* __TEST_FS_H */
