#include <stdio.h>
#include <stdlib.h>
#include "pal_log.h"
#include "pal_load_file.h"

void pal_load_file(const char *filename, load_file_t *load_file) {
    FILE *fp = fopen(filename, "rb");

    if (fp == NULL) {
        PAL_WARN("Warning: Failed to open file: %s", filename);
        return;
    }
    fseek(fp, 0, SEEK_END);
    long fsize = ftell(fp);

    fseek(fp, 0, SEEK_SET);
    load_file->buffer = malloc(fsize + 1);
    if (load_file->buffer == NULL) {
        PAL_WARN("Warning: Failed to malloc buffer for file: %s", filename);
        return;
    }
    fread(load_file->buffer, 1, fsize, fp);
    load_file->buffer[fsize] = '\0';
    load_file->size = fsize + 1;

    fclose(fp);
}
