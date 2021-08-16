#include <unistd.h>
#include <spawn.h>
#include <stdlib.h>
#include <errno.h>
#include <sys/wait.h>
#include <assert.h>

#include "test.h"

// ============================================================================
// Helper functions
// ============================================================================

static void handle_sigsegv(int num) {
    printf("SIGSEGV Caught in child with pid = %d, pgid = %d\n", getpid(), getpgid(0));
    assert(num == SIGSEGV);
    exit(0);
}

// Create a child process with different args which will have the pgid specified by `pgid`
// and the child will sleep and then abort.
// This new process should be killed to prevent aborting.
static int create_process_with_pgid(int pgid) {
    int ret = 0;
    posix_spawnattr_t attr;
    int child_pid = 0;

    // set child process spawn attribute
    ret = posix_spawnattr_init(&attr);
    if (ret != 0) {
        THROW_ERROR("init spawnattr error");
    }
    ret = posix_spawnattr_setflags(&attr, POSIX_SPAWN_SETPGROUP);
    if (ret != 0) {
        THROW_ERROR("set attribute flag error");
    }
    // child process will have its own process group
    ret = posix_spawnattr_setpgroup(&attr, pgid);
    if (ret != 0) {
        THROW_ERROR("set process group attribute error");
    }

    int child_argc = 2; // /bin/pgrp again
    char **child_argv = calloc(1, sizeof(char *) * (child_argc + 1));
    child_argv[0] = strdup("pgrp");
    child_argv[1] = strdup("again");
    ret = posix_spawn(&child_pid, "/bin/pgrp", NULL, &attr, child_argv, NULL);
    if (ret < 0) {
        THROW_ERROR("ERROR: failed to spawn a child process\n");
    }
    printf("Spawn a new proces successfully pid = %d\n", child_pid);
    posix_spawnattr_destroy(&attr);
    free(child_argv);
    return child_pid;
}

// ============================================================================
// Test cases for process group
// ============================================================================
int test_child_getpgid() {
    int ret, child_pid, status;
    int pgid = getpgid(0);
    int pgrp_id = getpgrp();
    if (pgid != pgrp_id) {
        THROW_ERROR("getpgrp error");
    }

    printf("Run a parent process with pid = %d, ppid = %d, pgid = %d\n", getpid(), getppid(),
           pgid);

    ret = posix_spawn(&child_pid, "/bin/getpid", NULL, NULL, NULL, NULL);
    if (ret < 0) {
        THROW_ERROR("ERROR: failed to spawn a child process\n");
    }
    printf("Spawn a child proces successfully with pid = %d\n", child_pid);

    // child process group should have same pgid with parent
    int child_pgid = getpgid(child_pid);
    if (child_pgid != pgid) {
        THROW_ERROR("child process group error");
    }

    ret = wait4(-1, &status, 0, NULL);
    if (ret < 0) {
        THROW_ERROR("ERROR: failed to wait4 the child process\n");
    }
    printf("Child process exited with status = %d\n", status);

    return 0;
}

int test_child_setpgid() {
    int ret, child_pid, status;

    printf("Parent process: pid = %d, ppid = %d, pgid = %d\n", getpid(), getppid(),
           getpgid(0));

    child_pid = create_process_with_pgid(0);
    if (child_pid < 0) {
        THROW_ERROR("create child process error");
    }

    // child pgid should be same as its pid
    int child_pgid = getpgid(child_pid);
    if (child_pgid != child_pid) {
        THROW_ERROR("child process group error");
    }

    kill(child_pid, SIGSEGV);
    ret = wait4(-1, &status, 0, NULL);
    if (ret < 0) {
        printf("ERROR: failed to wait4 the child process\n");
        return -1;
    }
    printf("Child process exited with status = %d\n", status);

    return 0;
}

int test_child_setpgid_to_other_child() {
    int ret, first_child_pid, second_child_pid, status;

    first_child_pid = create_process_with_pgid(0);
    if (first_child_pid < 0) {
        THROW_ERROR("failed to create first child");
    }

    // child pgid should be same as its pid
    int child_pgid = getpgid(first_child_pid);
    printf("first_child_pgid = %d\n", child_pgid);
    if (child_pgid != first_child_pid) {
        THROW_ERROR("first child process group error");
    }

    // add the second child to the first child's process group
    second_child_pid = create_process_with_pgid(child_pgid);
    if (second_child_pid < 0) {
        THROW_ERROR("failed to create first child");
    }

    // wait for child to run
    sleep(1);

    // second child pgid should be same as the the first child pgid
    int second_child_pgid = getpgid(second_child_pid);
    if (second_child_pgid != child_pgid) {
        THROW_ERROR("second child process group error");
    }
    kill(0 - second_child_pid, SIGSEGV);

    ret = kill(0 - child_pgid, SIGSEGV);
    if (ret < 0) {
        THROW_ERROR("ERROR: failed to kill process group 1\n");
    }

    // wait for all child process to exit
    while ((ret = wait(&status)) > 0);

    return 0;
}

int test_setpgid_to_running_child() {
    int ret, child_pid, status;

    ret = posix_spawn(&child_pid, "/bin/getpid", NULL, NULL, NULL, NULL);
    if (ret != 0) {
        THROW_ERROR("failed to spawn a child process");
    }

    // set child pgrp to itself
    if (setpgid(child_pid, 0) == 0 || errno != EACCES)  {
        THROW_ERROR("set child process group error not catching");
    }

    ret = wait4(-1, &status, 0, NULL);
    if (ret < 0) {
        THROW_ERROR("ERROR: failed to wait4 the child process\n");
    }

    return 0;
}

int test_setpgid_non_existent_pgrp() {
    int ret, child_pid;
    posix_spawnattr_t attr;
    int non_existent_pgid = 10;

    // make self process to join a non-existent process group
    if (setpgid(0, non_existent_pgid) == 0 || errno != EPERM ) {
        THROW_ERROR("set self process group error not catching");
    }

    // set child process group to a non-existent pgroup
    ret = posix_spawnattr_init(&attr);
    if (ret != 0) {
        THROW_ERROR("init spawnattr error");
    }
    ret = posix_spawnattr_setflags(&attr, POSIX_SPAWN_SETPGROUP);
    if (ret != 0) {
        THROW_ERROR("set attribute flag error");
    }
    ret = posix_spawnattr_setpgroup(&attr, non_existent_pgid);
    if (ret != 0) {
        THROW_ERROR("set process group attribute error");
    }
    ret = posix_spawn(&child_pid, "/bin/getpid", NULL, &attr, NULL, NULL);
    if (ret == 0 || errno != EPERM ) {
        THROW_ERROR("child process spawn error not catching\n");
    }

    //posix_spawn will fail. No need to wait for child.
    posix_spawnattr_destroy(&attr);
    return 0;
}

int test_signal_a_group_of_process() {
    printf("current(parent) pid = %d, pgid = %d\n", getpid(), getpgid(0));
    int process_group_1 = getpid();
    int ret, status;

    // spawn self with its own process group
    int child = create_process_with_pgid(0);
    if (child < 0) {
        THROW_ERROR("failed to create child");
    }
    int process_group_2 = child;

    // create 2 other children
    int other_children[2] = {0};
    int child_argc = 2; // /bin/pgrp again
    char **child_argv = calloc(1, sizeof(char *) * (child_argc + 1));
    child_argv[0] = strdup("pgrp");
    child_argv[1] = strdup("again");
    for (int i = 0; i < 2; i++) {
        ret = posix_spawn(&other_children[i], "/bin/pgrp", NULL, NULL, child_argv, NULL);
        if (ret < 0) {
            THROW_ERROR("ERROR: failed to spawn a child process\n");
        }
        printf("spawn other children pid = %d\n", other_children[i]);
    }
    free(child_argv);
    sleep(1);

    // make self process to join child's process group
    if (setpgid(0, process_group_2) < 0) {
        THROW_ERROR("join child process group error");
    }

    if (getpgid(0) != process_group_2) {
        THROW_ERROR("current pgid should be same as child's");
    }

    // other children should be in process group 1
    ret = kill(0 - process_group_1, SIGSEGV);
    if (ret < 0) {
        THROW_ERROR("ERROR: failed to kill process group 1\n");
    }

    // set a process group for self.
    // setpgrp() == setpgid(0,0)
    if (setpgrp() < 0) {
        THROW_ERROR("join child process group error");
    }

    ret = kill(0 - process_group_2, SIGSEGV);
    if (ret < 0) {
        THROW_ERROR("ERROR: failed to kill process group 2\n");
    }

    // wait for all child process to exit
    while ((ret = wait(&status)) > 0);

    return 0;
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_child_getpgid),
    TEST_CASE(test_child_setpgid),
    TEST_CASE(test_child_setpgid_to_other_child),
    TEST_CASE(test_setpgid_to_running_child),
    TEST_CASE(test_setpgid_non_existent_pgrp),
    TEST_CASE(test_signal_a_group_of_process),
};



int main(int argc, char **argv) {
    if (argc > 1) {
        // Spawn self. Do some extra work here.
        printf("pgrp run again as child with pid = %d, pgid = %d\n", getpid(), getpgid(0));
        signal(SIGSEGV, handle_sigsegv);
        sleep(10);
        // This shouldn't be reached.
        abort();
    }

    int ret;
    ret = test_suite_run(test_cases, ARRAY_SIZE(test_cases));
    return ret;
}
