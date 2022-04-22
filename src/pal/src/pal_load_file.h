#ifndef __PAL_LOAD_FILE_H__
#define __PAL_LOAD_FILE_H__

typedef struct {
    unsigned int size;
    char *buffer;
} load_file_t;

void pal_load_file(const char *filename, load_file_t *load_file);

#endif /* __PAL_LOAD_FILE_H__ */
