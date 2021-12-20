#include <unistd.h>
#include <string.h>
#include <stdio.h>
#include <stdlib.h>
#include <spawn.h>
#include <sys/wait.h>
#include <pthread.h>
#include <stdbool.h>
#include "test.h"

static void *just_sleep(void *_arg) {
    bool should_exit_by_execve = *(bool *)_arg;
    sleep(3);

    // If should_exit_by_execve is true, execve should be done before sleep returns.
    if (should_exit_by_execve) {
        printf("This should never be reached");
        exit(-1);
    } else {
        printf("sleep is done\n");
    }
    return NULL;
}

int test_execve_no_return(void) {
    bool should_exit_by_execve = true;
    pthread_t child_thread;
    if (pthread_create(&child_thread, NULL, just_sleep, (void *)&should_exit_by_execve) < 0) {
        THROW_ERROR("pthread_create failed");
    }

    char *args[] = {"spawn", NULL};
    execve("/bin/spawn", args, NULL);

    THROW_ERROR("The program shouldn't reach here.");
    return -1;
}

int test_execve_error_return(void) {
    // execve will fail in this case and thus the child thread will not exit until finish
    bool should_exit_by_execve = false;
    pthread_t child_thread;
    if (pthread_create(&child_thread, NULL, just_sleep, (void *)&should_exit_by_execve) < 0) {
        THROW_ERROR("pthread_create failed");
    }

    // during the time, try execve a non-exit process
    char *args[] = {"joke", NULL};
    int ret = execve("/bin/joke", args, NULL);
    if (ret != -1 || errno != ENOENT) {
        THROW_ERROR("execve error code wrong");
    }

    pthread_join(child_thread, NULL);
    return 0;
}

int test_execve_on_child_thread(void) {
    int ret, child_pid, status;

    // construct child process args
    int child_argc = 3; // ./nauty_child -t execve_thread
    char **child_argv = calloc(1, sizeof(char *) * (child_argc + 1));
    child_argv[0] = strdup("naughty_child");
    child_argv[1] = strdup("-t");
    child_argv[2] = strdup("execve_thread");

    ret = posix_spawn(&child_pid, "/bin/naughty_child", NULL, NULL, child_argv, NULL);
    if (ret != 0) {
        THROW_ERROR("failed to spawn a child process");
    }

    ret = waitpid(child_pid, &status, 0);
    if (ret < 0) {
        THROW_ERROR("failed to wait4 the child process");
    }
    printf("child process %d exit status = %d\n", child_pid, status);
    if (status != 0) {
        THROW_ERROR("child process exit with error");
    }

    return 0;
}

static test_case_t test_cases[] = {
    TEST_CASE(test_execve_on_child_thread),
    TEST_CASE(test_execve_error_return),
    TEST_CASE(test_execve_no_return),
};

int main() {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
