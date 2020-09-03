#define _GNU_SOURCE
#include <stdlib.h>
#include <unistd.h>
#include <stdio.h>
#include <pthread.h>
#include <string.h>
#include <sys/prctl.h>
#include <errno.h>
#include "test.h"

// ============================================================================
// Helper function
// ============================================================================
#define THREAD_NAME_LEN 16

extern char *program_invocation_short_name;
#define DEFAULT_NAME program_invocation_short_name // name of this executable

static const char *LONG_NAME = "A very very long thread name that is over 16 bytes";
static const char *NORMAL_NAME = "A thread name";

static int *test_thread_long_name(void *arg) {
    char thread_name[THREAD_NAME_LEN] = {0};
    char correct_name[THREAD_NAME_LEN] = {0};

    // Thread name can hold up to 16 bytes including null terminator
    // Construct the "correct_name" from the "long_name"
    strncpy(correct_name, LONG_NAME, THREAD_NAME_LEN - 1);
    correct_name[THREAD_NAME_LEN - 1] = '\0';

    if (prctl(PR_SET_NAME, LONG_NAME) != 0) {
        printf("long name test set thread name error\n");
        return (int *) -1;
    }
    if (prctl(PR_GET_NAME, thread_name) != 0) {
        printf("long name test set thread name error\n");
        return (int *) -1;
    }
    if (!strncmp(thread_name, correct_name, THREAD_NAME_LEN)) {
        return NULL;
    } else {
        printf("test long thread name mismatch\n");
        return (int *) -1;
    }
}

static int *test_thread_normal_name(void *arg) {
    char thread_name[THREAD_NAME_LEN] = {0};

    if (prctl(PR_SET_NAME, NORMAL_NAME) != 0) {
        printf("normal name test set thread name error\n");
        return (int *) -1;
    };
    if (prctl(PR_GET_NAME, thread_name) != 0) {
        printf("normal name test get thread name error\n");
        return (int *) -1;
    }
    if (!strncmp(thread_name, NORMAL_NAME, strlen(NORMAL_NAME))) {
        return NULL;
    } else {
        printf("test normal thread name mismatch\n");
        return (int *) -1;
    }
}

static int *test_thread_default_name(void *arg) {
    char thread_name[THREAD_NAME_LEN] = {0};

    if (prctl(PR_GET_NAME, thread_name) != 0) {
        printf("get thread default name error\n");
        return (int *) -1;
    }

    // The DEFAULT_NAME could be longer than THREAD_NAME_LEN and thus will make the last byte
    // to be the null-terminator. So we just compare length with "THREAD_NAME_LEN - 1"
    if (!strncmp(thread_name, DEFAULT_NAME, THREAD_NAME_LEN - 1)) {
        return NULL;
    } else {
        printf("test default thread name mismatch\n");
        return (int *) -1;
    }
}

// ============================================================================
// Test cases
// ============================================================================
static int test_prctl_set_get_long_name(void) {
    pthread_t tid;
    void *ret;

    if (pthread_create(&tid, NULL, (void *)test_thread_long_name, NULL))	{
        THROW_ERROR("create test long name thread failed");
    }
    pthread_join(tid, &ret);
    if ((int *) ret) {
        THROW_ERROR("test long name thread prctl error");
    }
    return 0;
}

static int test_prctl_set_get_normal_name(void) {
    pthread_t tid;
    void *ret;

    if (pthread_create(&tid, NULL, (void *)test_thread_normal_name, NULL))	{
        THROW_ERROR("create test normal name thread failed");
    }
    pthread_join(tid, &ret);
    if ((int *) ret) {
        THROW_ERROR("test normal name thread prctl error");
    }
    return 0;
}

static int test_prctl_get_default_thread_name(void) {
    pthread_t tid;
    void *ret;

    if (pthread_create(&tid, NULL, (void *)test_thread_default_name, NULL))	{
        THROW_ERROR("create test default name thread failed");
    }
    pthread_join(tid, &ret);
    if ((int *) ret) {
        THROW_ERROR("test default name thread prctl error");
    }
    return 0;
}

static int test_prctl_get_timerslack(void) {
    int nanoseconds = prctl(PR_GET_TIMERSLACK, 0, 0, 0, 0);
    if (nanoseconds < 0) {
        THROW_ERROR("test prctl get timer slack failed");
    };
    printf("timer slack = %d ns\n", nanoseconds);
    // Kernel default timer slack is 50us
    if (nanoseconds != 50000) {
        THROW_ERROR("timer slack is not 50us");
    }
    return 0;
}

// ============================================================================
// Test suite main
// ============================================================================
static test_case_t test_cases[] = {
    TEST_CASE(test_prctl_set_get_long_name), // over 16 bytes
    TEST_CASE(test_prctl_set_get_normal_name),
    TEST_CASE(test_prctl_get_default_thread_name),
    TEST_CASE(test_prctl_get_timerslack),
};

int main() {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
