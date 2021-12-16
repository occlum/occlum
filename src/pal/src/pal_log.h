#ifndef __PAL_LOG_H__
#define __PAL_LOG_H__

#include <stdio.h>

#define PAL_DEBUG(fmt, ...) \
    fprintf(stderr, "[DEBUG] occlum-pal: " fmt " (line %d, file %s)\n", ##__VA_ARGS__, __LINE__, __FILE__)
#define PAL_INFO(fmt, ...) \
    fprintf(stderr, "[INFO] occlum-pal: " fmt " (line %d, file %s)\n", ##__VA_ARGS__, __LINE__, __FILE__)
#define PAL_WARN(fmt, ...) \
    fprintf(stderr, "[WARN] occlum-pal: " fmt " (line %d, file %s)\n", ##__VA_ARGS__, __LINE__, __FILE__)
#define PAL_ERROR(fmt, ...) \
    fprintf(stderr, "[ERROR] occlum-pal: " fmt " (line %d, file %s)\n", ##__VA_ARGS__, __LINE__, __FILE__)

#endif /* __PAL_LOG_H__ */
