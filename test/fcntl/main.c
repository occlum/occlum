#include <errno.h>
#include <fcntl.h>
#include "test_fs.h"

// ============================================================================
// Helper macro
// ============================================================================

#define CREATION_FLAGS_MASK (O_CLOEXEC | O_CREAT| O_DIRECTORY | O_EXCL |    \
                             O_NOCTTY | O_NOFOLLOW | O_TMPFILE | O_TRUNC)

// ============================================================================
// Test cases for fcntl
// ============================================================================

static int __fcntl_getfl(int fd, int open_flags) {
    int actual_flags;

    actual_flags = fcntl(fd, F_GETFL);
    open_flags &= ~CREATION_FLAGS_MASK;
    open_flags |= O_LARGEFILE;
    if (open_flags != actual_flags) {
        THROW_ERROR("check getfl failed");
    }

    return 0;
}

static int __fcntl_setfl(int fd, int open_flags) {
    int ret, actual_flags;

    ret = fcntl(fd, F_SETFL, open_flags & ~O_APPEND);
    if (ret < 0) {
        THROW_ERROR("failed to call setfl");
    }
    actual_flags = fcntl(fd, F_GETFL);
    if ((actual_flags & O_APPEND) != 0) {
        THROW_ERROR("failed to check getfl after setfl");
    }

    return 0;
}

static int __fcntl_getlk_and_setlk(int fd, int open_flags) {
    int ret;
    struct flock fl = { F_WRLCK, SEEK_SET, 0, 0, 0 };

    // getlk
    ret = fcntl(fd, F_GETLK, &fl);
    if (ret < 0) {
        THROW_ERROR("failed to call getlk");
    }
    if (fl.l_type != F_UNLCK) {
        THROW_ERROR("failed to get correct fl type");
    }

    // setlk
    if ((open_flags & O_WRONLY) || (open_flags & O_RDWR)) {
        fl.l_type = F_WRLCK;
    } else {
        fl.l_type = F_RDLCK;
    }
    ret = fcntl(fd, F_SETLK, &fl);
    if (ret < 0) {
        THROW_ERROR("failed to call setlk");
    }

    return 0;
}

static int __fcntl_dupfd(int fd, int open_flags) {
    if (fcntl(fd, F_DUPFD, 0) < 0) {
        THROW_ERROR("failed to duplicate the fd");
    }
    return 0;
}

typedef int(*test_fcntl_func_t)(int fd, int open_flags);

static int test_fcntl_framework(test_fcntl_func_t fn) {
    const char *file_path = "/root/test_fcntl_file.txt";
    int open_flags = O_RDWR | O_CREAT | O_TRUNC | O_APPEND;
    int mode = 00666;
    int fd, ret;

    fd = open(file_path, open_flags, mode);
    if (fd < 0) {
        THROW_ERROR("failed to open & create file");
    }
    if (fn(fd, open_flags) < 0) {
        return -1;
    }
    close(fd);
    ret = unlink(file_path);
    if (ret < 0) {
        THROW_ERROR("failed to unlink the created file");
    }

    return 0;
}

static int test_fcntl_getfl() {
    return test_fcntl_framework(__fcntl_getfl);
}

static int test_fcntl_setfl() {
    return test_fcntl_framework(__fcntl_setfl);
}

static int test_getlk_and_setlk() {
    return test_fcntl_framework(__fcntl_getlk_and_setlk);
}

static int test_fcntl_dupfd() {
    return test_fcntl_framework(__fcntl_dupfd);
}

// ============================================================================
// Test suite
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_fcntl_getfl),
    TEST_CASE(test_fcntl_setfl),
    TEST_CASE(test_getlk_and_setlk),
    TEST_CASE(test_fcntl_dupfd),
};

int main() {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
