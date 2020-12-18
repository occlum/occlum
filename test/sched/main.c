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
// Helper function
// ============================================================================

#define MAX_CPU_NUM 1024

static int *g_online_cpu_idxs;

int get_online_cpu() {
    int online_num = sysconf(_SC_NPROCESSORS_ONLN);
    cpu_set_t mask;
    int index = 0;

    g_online_cpu_idxs = (int *)calloc(online_num, sizeof(int));
    CPU_ZERO(&mask);
    if (sched_getaffinity(0, sizeof(cpu_set_t), &mask) < 0) {
        THROW_ERROR("failed to call sched_getaffinity");
    }

    printf("Online Core No: ");
    for (int i = 0; index < online_num && i < MAX_CPU_NUM; i++) {
        if (CPU_ISSET(i, &mask)) {
            g_online_cpu_idxs[index] = i;
            index++;
            printf("%d ", i);
        }
    }
    printf("\n");
    return 0;
}

// ============================================================================
// Test cases for sched_cpu_affinity
// ============================================================================

static int test_sched_getaffinity_with_self_pid() {
    cpu_set_t mask;
    if (sched_getaffinity(0, sizeof(cpu_set_t), &mask) < 0) {
        THROW_ERROR("failed to call sched_getaffinity");
    }
    if (CPU_COUNT(&mask) <= 0) {
        THROW_ERROR("failed to get cpuset mask");
    }
    if (sysconf(_SC_NPROCESSORS_ONLN) < CPU_COUNT(&mask)) {
        THROW_ERROR("cpuset num must be less or equal to _SC_NPROCESSORS_ONLN");
    }
    return 0;
}

static int test_sched_setaffinity_with_self_pid() {
    int nproc = sysconf(_SC_NPROCESSORS_ONLN);
    cpu_set_t mask_old;
    CPU_ZERO(&mask_old);
    for (int i = 0; i < nproc; ++i) {
        CPU_SET(g_online_cpu_idxs[i], &mask_old);
    }
    cpu_set_t mask;
    CPU_ZERO(&mask);
    CPU_SET(g_online_cpu_idxs[0], &mask);
    if (sched_setaffinity(0, sizeof(cpu_set_t), &mask) < 0) {
        THROW_ERROR("failed to call sched_setaffinity \n");
    }
    cpu_set_t mask2;
    if (sched_getaffinity(0, sizeof(cpu_set_t), &mask2) < 0) {
        THROW_ERROR("failed to call sched_getaffinity");
    }
    if (!CPU_EQUAL(&mask, &mask2)) {
        THROW_ERROR("cpuset is wrong after get");
    }
    if (sched_setaffinity(0, sizeof(cpu_set_t), &mask_old) < 0) {
        THROW_ERROR("recover cpuset error");
    }
    return 0;
}

static int test_sched_xetaffinity_with_child_pid() {
    int status, child_pid;
    int num = sysconf(_SC_NPROCESSORS_ONLN);
    if (num <= 0) {
        THROW_ERROR("failed to get cpu number");
    }
    cpu_set_t mask;
    CPU_ZERO(&mask);
    CPU_SET(g_online_cpu_idxs[num - 1], &mask);
    int ret = posix_spawn(&child_pid, "/bin/getpid", NULL, NULL, NULL, NULL);
    if (ret < 0 ) {
        THROW_ERROR("spawn process error");
    }
    printf("Spawn a child process with pid=%d\n", child_pid);
    if (sched_setaffinity(child_pid, sizeof(cpu_set_t), &mask) < 0) {
        THROW_ERROR("failed to set child affinity");
    }
    cpu_set_t mask2;
    if (sched_getaffinity(child_pid, sizeof(cpu_set_t), &mask2) < 0) {
        THROW_ERROR("failed to get child affinity");
    }
    if (!CPU_EQUAL(&mask, &mask2)) {
        THROW_ERROR("cpuset is wrong in child");
    }
    ret = wait4(-1, &status, 0, NULL);
    if (ret < 0) {
        THROW_ERROR("failed to wait4 the child proces");
    }
    return 0;
}

static int test_sched_xetaffinity_children_inheritance() {
    int status, child_pid;
    int num_core = sysconf(_SC_NPROCESSORS_ONLN);
    if (num_core <= 0) {
        THROW_ERROR("failed to get cpu number");
    }
    cpu_set_t mask;
    CPU_ZERO(&mask);
    CPU_SET(g_online_cpu_idxs[num_core - 1], &mask);
    if (sched_setaffinity(0, sizeof(cpu_set_t), &mask) < 0) {
        THROW_ERROR("failed to set parent affinity");
    }

    int ret = posix_spawn(&child_pid, "/bin/getpid", NULL, NULL, NULL, NULL);
    if (ret < 0 ) {
        THROW_ERROR("spawn process error");
    }
    printf("Spawn a child process with pid=%d\n", child_pid);

    cpu_set_t mask2;
    if (sched_getaffinity(child_pid, sizeof(cpu_set_t), &mask2) < 0) {
        THROW_ERROR("failed to get child affinity");
    }
    if (!CPU_EQUAL(&mask, &mask2)) {
        THROW_ERROR("affinity inherited from parent is wrong in child");
    }

    // Set affinity to child should not affect parent process
    CPU_SET(g_online_cpu_idxs[0], &mask2);
    if (sched_setaffinity(child_pid, sizeof(cpu_set_t), &mask2) < 0) {
        THROW_ERROR("failed to set child affinity");
    }

    CPU_ZERO(&mask2);
    if (sched_getaffinity(0, sizeof(cpu_set_t), &mask2) < 0) {
        THROW_ERROR("failed to get parent process affinity");
    }

    if (!CPU_EQUAL(&mask, &mask2)) {
        THROW_ERROR("cpuset is wrong in parent process");
    }

    ret = wait4(-1, &status, 0, NULL);
    if (ret < 0) {
        THROW_ERROR("failed to wait4 the child procces");
    }
    return 0;
}

#define CPU_SET_SIZE_LIMIT (128)

static int test_sched_getaffinity_via_explicit_syscall() {
    unsigned char buf[CPU_SET_SIZE_LIMIT] = { 0 };
    int ret = syscall(__NR_sched_getaffinity, 0, CPU_SET_SIZE_LIMIT, buf);
    if (ret <= 0) {
        THROW_ERROR("failed to call __NR_sched_getaffinity");
    }
    return 0;
}

static int test_sched_setaffinity_via_explicit_syscall() {
    cpu_set_t mask;
    CPU_ZERO(&mask);
    CPU_SET(g_online_cpu_idxs[0], &mask);
    if (syscall(__NR_sched_setaffinity, 0, sizeof(cpu_set_t), &mask) < 0) {
        THROW_ERROR("failed to call __NR_sched_setaffinity");
    }

    cpu_set_t mask2;
    CPU_ZERO(&mask2);
    int ret_nproc = syscall(__NR_sched_getaffinity, 0, sizeof(cpu_set_t), &mask2);
    if (ret_nproc <= 0) {
        THROW_ERROR("failed to call __NR_sched_getaffinity");
    }
    if (!CPU_EQUAL(&mask, &mask2)) {
        THROW_ERROR("explicit syscall cpuset is wrong");
    }

    // Recover the affinity mask
    int nproc = sysconf(_SC_NPROCESSORS_ONLN);
    cpu_set_t mask_old;
    CPU_ZERO(&mask_old);
    for (int i = 0; i < nproc; ++i) {
        CPU_SET(g_online_cpu_idxs[i], &mask_old);
    }
    if (syscall(__NR_sched_setaffinity, 0, sizeof(cpu_set_t), &mask_old) < 0) {
        THROW_ERROR("recover cpuset error");
    }
    return 0;
}

static int test_sched_getaffinity_with_zero_cpusetsize() {
    cpu_set_t mask;
    if (sched_getaffinity(0, 0, &mask) != -1) {
        THROW_ERROR("check invalid cpusetsize(0) fail");
    }
    return 0;
}

static int test_sched_setaffinity_with_zero_cpusetsize() {
    cpu_set_t mask;
    if (sched_setaffinity(0, 0, &mask) != -1) {
        THROW_ERROR("check invalid cpusetsize(0) fail");
    }
    return 0;
}

static int test_sched_getaffinity_with_null_buffer() {
    unsigned char *buf = NULL;
    if (sched_getaffinity(0, sizeof(cpu_set_t), (cpu_set_t *)buf) != -1) {
        THROW_ERROR("check invalid buffer pointer(NULL) fail");
    }
    return 0;
}

static int test_sched_setaffinity_with_null_buffer() {
    unsigned char *buf = NULL;
    if (sched_setaffinity(0, sizeof(cpu_set_t), (cpu_set_t *)buf) != -1) {
        THROW_ERROR("check invalid buffer pointer(NULL) fail");
    }
    return 0;
}

// ============================================================================
// Test cases for sched_yield
// ============================================================================

static int test_sched_yield() {
    // In the Linux implementation, sched_yield() always succeeds.
    if (sched_yield() < 0) {
        THROW_ERROR("check sched yield fail");
    }
    return 0;
}

// ============================================================================
// Test cases for getcpu
// ============================================================================

static int test_getcpu() {
    int cpu, node;
    if (syscall(__NR_getcpu, &cpu, &node, NULL) < 0) {
        THROW_ERROR("getcpu with cpu&node fail");
    }
    if (syscall(__NR_getcpu, &cpu, NULL, NULL) < 0) {
        THROW_ERROR("getcpu with cpu fail");
    }
    if (syscall(__NR_getcpu, NULL, &node, NULL) < 0) {
        THROW_ERROR("getcpu with node fail");
    }
    if (syscall(__NR_getcpu, NULL, NULL, NULL) < 0) {
        THROW_ERROR("getcpu with null fail");
    }
    return 0;
}

static int test_getcpu_after_setaffinity() {
    int nproc = sysconf(_SC_NPROCESSORS_ONLN);
    cpu_set_t mask_old;
    CPU_ZERO(&mask_old);
    for (int i = 0; i < nproc; ++i) {
        CPU_SET(g_online_cpu_idxs[i], &mask_old);
    }

    cpu_set_t mask;
    CPU_ZERO(&mask);
    CPU_SET(g_online_cpu_idxs[0], &mask);
    if (sched_setaffinity(0, sizeof(cpu_set_t), &mask) < 0) {
        THROW_ERROR("failed to call sched_setaffinity \n");
    }

    int cpu;
    int ret = syscall(__NR_getcpu, &cpu, NULL, NULL);
    if (ret < 0) {
        THROW_ERROR("getcpu fail");
    }
    if (cpu != g_online_cpu_idxs[0]) {
        THROW_ERROR("check processor id fail");
    }

    if (sched_setaffinity(0, sizeof(cpu_set_t), &mask_old) < 0) {
        THROW_ERROR("recover cpuset error");
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
    TEST_CASE(test_sched_yield),
    TEST_CASE(test_sched_xetaffinity_children_inheritance),
    TEST_CASE(test_getcpu),
    TEST_CASE(test_getcpu_after_setaffinity),
};

int main() {
    int ret;
    get_online_cpu();
    ret = test_suite_run(test_cases, ARRAY_SIZE(test_cases));
    free(g_online_cpu_idxs);
    return ret;
}
