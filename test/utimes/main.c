#define _GNU_SOURCE
#include <sys/types.h>
#include <sys/time.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <fcntl.h>
#include <time.h>
#include <unistd.h>
#include <utime.h>
#include "test_fs.h"

// ============================================================================
// Global variables
// ============================================================================

static int SUCCESS = 1;
static int FAIL = -1;
const static struct timespec period_of_100ms = {
    .tv_sec = 0,
    .tv_nsec = 100 * (1000 * 1000)
};

// ============================================================================
// Helper function
// ============================================================================

static int create_file(const char *file_path) {
    int fd;
    int flags = O_RDONLY | O_CREAT | O_TRUNC;
    int mode = 00444;

    fd = open(file_path, flags, mode);
    if (fd < 0) {
        THROW_ERROR("failed to create a file");
    }
    close(fd);
    return 0;
}

static int remove_file(const char *file_path) {
    int ret;

    ret = unlink(file_path);
    if (ret < 0) {
        THROW_ERROR("failed to unlink the created file");
    }
    return 0;
}

// ============================================================================
// Test cases for utime
// ============================================================================


static int __test_utime(const char *file_path) {
    struct stat stat_buf;
    struct utimbuf times;
    struct timeval timeval;
    time_t actime, modtime;
    int ret;

    ret = stat(file_path, &stat_buf);
    if (ret < 0) {
        THROW_ERROR("failed to stat file");
    }
    actime = stat_buf.st_atim.tv_sec + 1;
    modtime = stat_buf.st_mtim.tv_sec + 2;
    times.actime = actime;
    times.modtime = modtime;
    ret = syscall(SYS_utime, file_path, &times);
    if (ret < 0) {
        THROW_ERROR("failed to utime file");
    }
    ret = stat(file_path, &stat_buf);
    if (ret < 0) {
        THROW_ERROR("failed to stat file");
    }
    if (stat_buf.st_atim.tv_sec != actime ||
            stat_buf.st_atim.tv_nsec != 0 ||
            stat_buf.st_mtim.tv_sec != modtime ||
            stat_buf.st_mtim.tv_nsec != 0) {
        THROW_ERROR("check utime result failed");
    }

    // If times is NULL, then the access and modification times
    // of the file are set to the current time.
    ret = gettimeofday(&timeval, NULL);
    if (ret < 0) {
        THROW_ERROR("failed to gettimeofday");
    }
    ret = syscall(SYS_utime, file_path, NULL);
    if (ret < 0) {
        THROW_ERROR("failed to utime file");
    }

    ret = stat(file_path, &stat_buf);
    if (ret < 0) {
        THROW_ERROR("failed to stat file");
    }

    if (stat_buf.st_atim.tv_sec != timeval.tv_sec ||
            stat_buf.st_mtim.tv_sec != timeval.tv_sec) {
        THROW_ERROR("check utime result failed");
    }

    return SUCCESS;
}

static int __test_utimes(const char *file_path) {
    struct stat stat_buf;
    struct timeval actime, modtime, times[2];
    int ret;

    nanosleep(&period_of_100ms, NULL);
    gettimeofday(&actime, NULL);
    nanosleep(&period_of_100ms, NULL);
    gettimeofday(&modtime, NULL);
    times[0] = actime;
    times[1] = modtime;
    ret = syscall(SYS_utimes, file_path, times);
    if (ret < 0) {
        THROW_ERROR("failed to utime file");
    }
    ret = stat(file_path, &stat_buf);
    if (ret < 0) {
        THROW_ERROR("failed to stat file");
    }
    if (stat_buf.st_atim.tv_sec != actime.tv_sec ||
            stat_buf.st_atim.tv_nsec / 1000 != actime.tv_usec ||
            stat_buf.st_mtim.tv_sec != modtime.tv_sec ||
            stat_buf.st_mtim.tv_nsec / 1000 != modtime.tv_usec) {
        THROW_ERROR("check utimes result failed");
    }

    return SUCCESS;
}

static int __test_futimesat(const char *file_path) {
    struct stat stat_buf;
    struct timeval actime, modtime, times[2];
    char dir_buf[PATH_MAX] = { 0 };
    char base_buf[PATH_MAX] = { 0 };
    char *dir_name, *file_name;
    int dirfd, ret;

    if (fs_split_path(file_path, dir_buf, &dir_name, base_buf, &file_name) < 0) {
        THROW_ERROR("failed to split path");
    }
    dirfd = open(dir_name, O_RDONLY);
    if (dirfd < 0) {
        THROW_ERROR("failed to split path");
    }
    nanosleep(&period_of_100ms, NULL);
    gettimeofday(&actime, NULL);
    nanosleep(&period_of_100ms, NULL);
    gettimeofday(&modtime, NULL);
    times[0] = actime;
    times[1] = modtime;
    ret = syscall(SYS_futimesat, dirfd, file_name, times);
    if (ret < 0) {
        THROW_ERROR("failed to futimesat file with dirfd");
    }
    close(dirfd);
    ret = stat(file_path, &stat_buf);
    if (ret < 0) {
        THROW_ERROR("failed to stat file");
    }
    if (stat_buf.st_atim.tv_sec != actime.tv_sec ||
            stat_buf.st_atim.tv_nsec / 1000 != actime.tv_usec ||
            stat_buf.st_mtim.tv_sec != modtime.tv_sec ||
            stat_buf.st_mtim.tv_nsec / 1000 != modtime.tv_usec) {
        THROW_ERROR("check utimes result failed");
    }

    return SUCCESS;
}

static int __test_futimesat_nullpath(const char *file_path) {
    struct stat stat_buf;
    struct timeval actime, modtime, times[2];
    int dirfd, ret;

    dirfd = open(file_path, O_RDONLY);
    if (dirfd < 0) {
        THROW_ERROR("failed to open dir");
    }
    nanosleep(&period_of_100ms, NULL);
    gettimeofday(&actime, NULL);
    nanosleep(&period_of_100ms, NULL);
    gettimeofday(&modtime, NULL);
    times[0] = actime;
    times[1] = modtime;
    ret = syscall(SYS_futimesat, dirfd, NULL, times);
    if (ret < 0) {
        THROW_ERROR("failed to futimesat file with dirfd");
    }
    close(dirfd);
    ret = stat(file_path, &stat_buf);
    if (ret < 0) {
        THROW_ERROR("failed to stat file");
    }
    if (stat_buf.st_atim.tv_sec != actime.tv_sec ||
            stat_buf.st_atim.tv_nsec / 1000 != actime.tv_usec ||
            stat_buf.st_mtim.tv_sec != modtime.tv_sec ||
            stat_buf.st_mtim.tv_nsec / 1000 != modtime.tv_usec) {
        THROW_ERROR("check utimes result failed");
    }
    return SUCCESS;
}

static int __test_utimensat(const char *file_path) {
    struct stat stat_buf;
    struct timespec actime, modtime, times[2];
    char dir_buf[PATH_MAX] = { 0 };
    char base_buf[PATH_MAX] = { 0 };
    char *dir_name, *file_name;
    int dirfd, ret;

    if (fs_split_path(file_path, dir_buf, &dir_name, base_buf, &file_name) < 0) {
        THROW_ERROR("failed to split path");
    }
    dirfd = open(dir_name, O_RDONLY);
    if (dirfd < 0) {
        THROW_ERROR("failed to open dir");
    }
    nanosleep(&period_of_100ms, NULL);
    clock_gettime(CLOCK_REALTIME, &actime);
    nanosleep(&period_of_100ms, NULL);
    clock_gettime(CLOCK_REALTIME, &modtime);
    times[0] = actime;
    times[1] = modtime;
    ret = syscall(SYS_utimensat, dirfd, file_name, times, 0);
    if (ret < 0) {
        THROW_ERROR("failed to futimesat file with dirfd");
    }
    close(dirfd);
    ret = stat(file_path, &stat_buf);
    if (ret < 0) {
        THROW_ERROR("failed to stat file");
    }
    if (stat_buf.st_atim.tv_sec != actime.tv_sec ||
            stat_buf.st_atim.tv_nsec != actime.tv_nsec ||
            stat_buf.st_mtim.tv_sec != modtime.tv_sec ||
            stat_buf.st_mtim.tv_nsec != modtime.tv_nsec) {
        THROW_ERROR("check utimes result failed");
    }
    return SUCCESS;
}

static int __test_utimensat_invalid_flag(const char *file_path) {
    struct timespec times[2] = {{10, 0}, {20, 0}};
    char dir_buf[PATH_MAX] = { 0 };
    char base_buf[PATH_MAX] = { 0 };
    char *dir_name, *file_name;
    int dirfd, ret;

    if (fs_split_path(file_path, dir_buf, &dir_name, base_buf, &file_name) < 0) {
        THROW_ERROR("failed to split path");
    }
    dirfd = open(dir_name, O_RDONLY);
    if (dirfd < 0) {
        THROW_ERROR("failed to open dir");
    }
    // AT_SYMLINK_NOFOLLOW is invalid if we modify timestamps of the file
    // referred to by the file descriptor dirfd
    ret = syscall(SYS_utimensat, dirfd, NULL, times, AT_SYMLINK_NOFOLLOW);
    if (ret != -1 && errno != EINVAL) {
        THROW_ERROR("utimnsat() should return EINVAL");
    }
    close(dirfd);

    return SUCCESS;
}

typedef int(*test_utimes_func_t)(const char *);

static int test_utimes_framework(test_utimes_func_t fn) {
    const char *file_path = "/root/test_filesystem_utimes.txt";

    if (create_file(file_path) < 0) {
        return FAIL;
    }
    if (fn(file_path) < 0) {
        return FAIL;
    }
    if (remove_file(file_path) < 0) {
        return FAIL;
    }
    return SUCCESS;
}

static int test_utime() {
    return test_utimes_framework(__test_utime);
}

static int test_utimes() {
    return test_utimes_framework(__test_utimes);
}

static int test_futimesat() {
    return test_utimes_framework(__test_futimesat);
}

static int test_futimesat_nullpath() {
    return test_utimes_framework(__test_futimesat_nullpath);
}

static int test_utimensat() {
    return test_utimes_framework(__test_utimensat);
}

static int test_utimensat_invalid_flag() {
    return test_utimes_framework(__test_utimensat_invalid_flag);
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_utime),
    TEST_CASE(test_utimes),
    TEST_CASE(test_futimesat),
    TEST_CASE(test_futimesat_nullpath),
    TEST_CASE(test_utimensat),
    TEST_CASE(test_utimensat_invalid_flag),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
