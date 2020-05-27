#include <stdio.h>
#include "ocalls.h"

typedef enum {
    LEVEL_OFF   = 0,
    LEVEL_ERROR = 1,
    LEVEL_WARN  = 2,
    LEVEL_INFO  = 3,
    LEVEL_DEBUG = 4,
    LEVEL_TRACE = 5
} level_t;

#define COLOR_NORMAL    "\x1B[0m"
#define COLOR_RED       "\x1B[31m"
#define COLOR_YELLOW    "\x1B[33m"
#define COLOR_GREEN     "\x1B[32m"

static level_t new_level(unsigned int level) {
    if (level >= 5) { level = 5; }
    return (level_t) level;
}

void occlum_ocall_print_log(unsigned int _level, const char *msg) {
    level_t level = new_level(_level);
    if (level == LEVEL_OFF) {
        return;
    }

    const char *color;
    switch (level) {
        case LEVEL_ERROR:
            color = COLOR_RED;
            break;
        case LEVEL_WARN:
            color = COLOR_YELLOW;
            break;
        case LEVEL_INFO:
            color = COLOR_GREEN;
            break;
        default:
            color = COLOR_NORMAL;
    }

    struct timeval now_tv;
    gettimeofday(&now_tv, NULL);
    char day_and_sec[20];
    strftime(day_and_sec, 20, "%Y-%m-%dT%H:%M:%S", gmtime(&now_tv.tv_sec));
    int ms = now_tv.tv_usec / 1000;
    fprintf(stderr, "%s[%s.%03dZ]%s%s\n", color, day_and_sec, ms, msg, COLOR_NORMAL);
}

void occlum_ocall_flush_log() {
    fflush(stderr);
}
