#include <stdio.h>
#include <stdlib.h>
#include "pal_log.h"

char *pal_load_file(const char *filename) {
    FILE *fp = fopen(filename, "rb");

    if (fp == NULL) {
        PAL_WARN("Warning: Failed to open file: %s", filename);
        return NULL;
    }
    fseek(fp, 0, SEEK_END);
    long fsize = ftell(fp);
    fseek(fp, 0, SEEK_SET);
    char *file_buffer = malloc(fsize + 1);
    if (file_buffer == NULL) {
        PAL_WARN("Warning: Failed to malloc buffer for file: %s", filename);
        return NULL;
    }
    fread(file_buffer, 1, fsize, fp);
    file_buffer[fsize] = 0;
    fclose(fp);
    return file_buffer;
}
