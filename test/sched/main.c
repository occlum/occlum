#define _GNU_SOURCE
#include <unistd.h>
#include <assert.h>
#include <string.h>
#include <stdio.h>
#include <stdlib.h>
#include <sched.h>
#include <errno.h>
#include <spawn.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include "test.h"

// ============================================================================
// Test cases for sched_cpu_affinity
// ============================================================================

static int test_sched_getaffinity_with_self_pid() {
    cpu_set_t mask;
    if (sched_getaffinity(0, sizeof(cpu_set_t), &mask) < 0) {
        throw_error("failed to call sched_getaffinity");
    }
    if (CPU_COUNT(&mask) <= 0) {
        throw_error("failed to get cpuset mask");
    }
    if (sysconf(_SC_NPROCESSORS_ONLN) != CPU_COUNT(&mask)) {
        throw_error("cpuset num wrong");
    }
    return 0;
}

static int test_sched_setaffinity_with_self_pid() {
    int nproc = sysconf(_SC_NPROCESSORS_ONLN);
    cpu_set_t mask_old;
    for (int i = 0; i < nproc; ++i) {
        CPU_SET(i, &mask_old);
    }
    cpu_set_t mask;
    CPU_ZERO(&mask);
    CPU_SET(0, &mask);
    if (sched_setaffinity(0, sizeof(cpu_set_t), &mask) < 0) {
        throw_error("failed to call sched_setaffinity \n");
    }
    cpu_set_t mask2;
    if (sched_getaffinity(0, sizeof(cpu_set_t), &mask2) < 0) {
        throw_error("failed to call sched_getaffinity");
    }
    if (!CPU_EQUAL(&mask, &mask2)) {
        throw_error("cpuset is wrong after get");
    }
    if (sched_setaffinity(0, sizeof(cpu_set_t), &mask_old) < 0) {
        throw_error("recover cpuset error");
    }
    return 0;
}

static int test_sched_xetaffinity_with_child_pid() {
    int status, child_pid;
    int num = sysconf(_SC_NPROCESSORS_CONF);
    if (num <= 0) {
        throw_error("failed to get cpu number");
    }
    cpu_set_t mask;
    CPU_ZERO(&mask);
    CPU_SET(num - 1 , &mask);
    int ret = posix_spawn(&child_pid, "getpid", NULL, NULL, NULL, NULL);
    if (ret < 0 ) {
        throw_error("spawn process error");
    }
    printf("Spawn a child process with pid=%d\n", child_pid);
    if (sched_setaffinity(child_pid, sizeof(cpu_set_t), &mask) < 0) {
        throw_error("failed to set child affinity");
    }
    cpu_set_t mask2;
    if (sched_getaffinity(child_pid, sizeof(cpu_set_t), &mask2) < 0) {
        throw_error("failed to get child affinity");
    }
    if (!CPU_EQUAL(&mask, &mask2)) {
        throw_error("cpuset is wrong in child");
    }
    ret = wait4(-1, &status, 0, NULL);
    if (ret < 0) {
        throw_error("failed to wait4 the child proces");
    }
    return 0;
}

#define CPU_SET_SIZE_LIMIT (1024)

static int test_sched_getaffinity_via_explicit_syscall() {
    unsigned char buf[CPU_SET_SIZE_LIMIT] = { 0 };
    int ret = syscall(__NR_sched_getaffinity, 0, CPU_SET_SIZE_LIMIT, buf);
    if (ret <= 0) {
        throw_error("failed to call __NR_sched_getaffinity");
    }
    return 0;
}

static int test_sched_setaffinity_via_explicit_syscall() {
    int nproc = sysconf(_SC_NPROCESSORS_ONLN);
    cpu_set_t mask_old;
    for (int i = 0; i < nproc; ++i) {
        CPU_SET(i, &mask_old);
    }
    cpu_set_t mask;
    CPU_ZERO(&mask);
    CPU_SET(0, &mask);
    if (syscall(__NR_sched_setaffinity, 0, sizeof(cpu_set_t), &mask) < 0) {
        throw_error("failed to call __NR_sched_setaffinity");
    }
    cpu_set_t mask2;
    int ret_nproc = syscall(__NR_sched_getaffinity, 0, sizeof(cpu_set_t), &mask2);
    if (ret_nproc <= 0) {
        throw_error("failed to call __NR_sched_getaffinity");
    }
    if (!CPU_EQUAL(&mask, &mask2)) {
        throw_error("explicit syscall cpuset is wrong");
    }
    if (syscall(__NR_sched_setaffinity, 0, sizeof(cpu_set_t), &mask_old) < 0) {
        throw_error("recover cpuset error");
    }
    return 0;
}

static int test_sched_getaffinity_with_zero_cpusetsize() {
    cpu_set_t mask;
    if (sched_getaffinity(0, 0, &mask) != -1) {
        throw_error("check invalid cpusetsize(0) fail");
    }
    return 0;
}

static int test_sched_setaffinity_with_zero_cpusetsize() {
    cpu_set_t mask;
    if (sched_setaffinity(0, 0, &mask) != -1) {
        throw_error("check invalid cpusetsize(0) fail");
    }
    return 0;
}

static int test_sched_getaffinity_with_null_buffer() {
    unsigned char *buf = NULL;
    if (sched_getaffinity(0, sizeof(cpu_set_t), (cpu_set_t*)buf) != -1) {
        throw_error("check invalid buffer pointer(NULL) fail");
    }
    return 0;
}

static int test_sched_setaffinity_with_null_buffer() {
    unsigned char *buf = NULL;
    if (sched_setaffinity(0, sizeof(cpu_set_t), (cpu_set_t*)buf) != -1) {
        throw_error("check invalid buffer pointer(NULL) fail");
    }
    return 0;
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_sched_xetaffinity_with_child_pid),
    TEST_CASE(test_sched_getaffinity_with_self_pid),
    TEST_CASE(test_sched_setaffinity_with_self_pid),
    TEST_CASE(test_sched_getaffinity_via_explicit_syscall),
    TEST_CASE(test_sched_setaffinity_via_explicit_syscall),
    TEST_CASE(test_sched_getaffinity_with_zero_cpusetsize),
    TEST_CASE(test_sched_setaffinity_with_zero_cpusetsize),
    TEST_CASE(test_sched_getaffinity_with_null_buffer),
    TEST_CASE(test_sched_setaffinity_with_null_buffer),
};

int main() {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
