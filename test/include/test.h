#ifndef __TEST_H
#define __TEST_H

#include <stdio.h>
#include <stdarg.h>
#include <unistd.h>
#include <errno.h>
#include <string.h>

#define _STR(x)             #x
#define STR(x)              _STR(x)
#define MIN(a, b)               ((a) <= (b) ? (a) : (b))
#define MAX(a, b)               ((a) >= (b) ? (a) : (b))
#define ARRAY_SIZE(array)   (sizeof(array)/sizeof(array[0]))

typedef int(*test_case_func_t)(void);

typedef struct {
    const char             *name;
    test_case_func_t        func;
} test_case_t;

#define TEST_CASE(name)     { STR(name), name }

#define THROW_ERROR(fmt, ...)   do { \
    printf("\t\tERROR:" fmt " in func %s at line %d of file %s with errno %d: %s\n", \
    ##__VA_ARGS__, __func__, __LINE__, __FILE__, errno, strerror(errno)); \
    return -1; \
} while (0)

int test_suite_run(test_case_t *test_cases, int num_test_cases) {
    for (int ti = 0; ti < num_test_cases; ti++) {
        test_case_t *tc = &test_cases[ti];
        if (tc->func() < 0) {
            printf("  func %s - [ERR]\n", tc->name);
            return -1;
        }
        printf("  func %s - [OK]\n", tc->name);
    }
    return 0;
}

void close_files(int count, ...) {
    va_list ap;
    va_start(ap, count);
    for (int i = 0; i < count; i++) {
        close(va_arg(ap, int));
    }
    va_end(ap);
}

int check_bytes_in_buf(char *buf, size_t len, int expected_byte_val) {
    for (size_t bi = 0; bi < len; bi++) {
        if (buf[bi] != (char)expected_byte_val) {
            THROW_ERROR("check_bytes_in_buf: expect %02X, but found %02X, at offset %lu\n",
                        (unsigned char)expected_byte_val, (unsigned char)buf[bi], bi);
        }
    }
    return 0;
}

#endif /* __TEST_H */
