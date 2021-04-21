#include <sys/utsname.h>
#include <stdio.h>
#include <unistd.h>
#include <stdlib.h>
#include "test.h"

static int test_uname() {
    struct utsname name;
    uname(&name);
    printf("sysname = %s\n", (const char *)&name.sysname);
    printf("nodename = %s\n", (const char *)&name.nodename);
    printf("release = %s\n", (const char *)&name.release);
    printf("version = %s\n", (const char *)&name.version);
    printf("machine = %s\n", (const char *)&name.machine);
    printf("domainname = %s\n", (const char *)&name.__domainname);

    return 0;
}

static int test_getgroups() {
    int group_num = getgroups(0, NULL);
    if (group_num != 1) {
        THROW_ERROR("getgroups failed to get size");
    }

    gid_t group_list[1] = {1};

    group_num = getgroups(group_num, group_list);

    printf("group_num %d group %d\n", group_num, group_list[0]);
    if (group_num != 1 || group_list[0] != 0) {
        THROW_ERROR("getgroups failed to get group_list");
    }
    return 0;
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_uname),
    TEST_CASE(test_getgroups),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
