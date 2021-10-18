#include <stdio.h>
#include <unistd.h>
#include <sys/sysinfo.h>
#include <spawn.h>
#include <sys/wait.h>
#include "test.h"

int test_sysinfo() {
    const long MIN = 60;
    const long HOUR = MIN * 60;
    const long DAY = HOUR * 24;
    const double MEGABYTE = 1024 * 1024;
    struct sysinfo info;
    int child_pid, status;

    // Test uptime
    sleep(3);

    // Test procs number
    int ret = posix_spawn(&child_pid, "/bin/getpid", NULL, NULL, NULL, NULL);
    if (ret < 0 ) {
        THROW_ERROR("spawn process error");
    }

    sysinfo (&info);

    printf ("system uptime: %ld days, %ld:%02ld:%02ld\n",
            info.uptime / DAY, (info.uptime % DAY) / HOUR,
            (info.uptime % HOUR) / MIN, info.uptime % MIN);
    printf ("total RAM: %5.1f MB\n", info.totalram / MEGABYTE);
    printf ("free RAM: %5.1f MB\n", info.freeram / MEGABYTE);
    printf ("process count: %d\n", info.procs);

    // make sure update is in a valid range ( > 1s)
    if (info.uptime < 1) {
        THROW_ERROR("system uptime error");
    }

    if (info.procs != 2 ) {
        THROW_ERROR("system process count error");
    }

    ret = wait4(-1, &status, 0, NULL);
    if (ret < 0) {
        THROW_ERROR("failed to wait4 the child proces");
    }

    return 0;
}

static test_case_t test_cases[] = {
    TEST_CASE(test_sysinfo),
};

int main() {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
