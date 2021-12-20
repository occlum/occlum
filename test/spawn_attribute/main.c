#include <spawn.h>
#include <signal.h>
#include <pthread.h>
#include <assert.h>
#include <sys/wait.h>
#include <stdlib.h>
#include "test.h"

void sigchld_handler(int sig) {
    printf("SIGCHLD is caught in father process!\n");
}

void sigio_handler(int sig) {
    printf("SIGIO is caught in father process!\n");
}

static void *thread_func(void *_arg) {
#ifndef __GLIBC__
    // musl can perform extra checks
    // child thread sigmask should be same with father thread
    struct __sigset_t *father_thread_mask = (struct __sigset_t *)_arg;
    struct __sigset_t current_mask;
    sigprocmask(0, NULL, &current_mask);
    assert(father_thread_mask->__bits[0] == current_mask.__bits[0]);
    printf("[child thread] father: %ld, child: %ld\n", father_thread_mask->__bits[0],
           current_mask.__bits[0]);
#endif

    // SIGIO is IGNORED and shouldn't be handled
    raise(SIGIO);
    printf("[child thread] SIGIO is ignored\n");
    raise(SIGABRT);
    printf("[child thread] SIGABRT is sigmasked\n");

    // change sigmask in child thread and monitor in father thread
#ifndef __GLIBC__
    struct __sigset_t new_sigmask;
#else
    sigset_t new_sigmask;
#endif
    sigemptyset(&new_sigmask);
    sigaddset(&new_sigmask, SIGALRM);
    sigprocmask(SIG_BLOCK, &new_sigmask, NULL);

    // change SIGIO sigaction in child thread and monitor in father thread
    signal(SIGIO, sigio_handler);
    printf("[child thread] SIGIO handler is changed\n");
    return NULL;
}

// Each thread of a process has its own sigmask but a process has the same sigaction for different thread。
// Father thread set SIGIO to SIG_IGN, and block SIGABRT. Child thread inherits the sigmask and sigaction and
// change both sigmask and sigaction. Father thread's sigmask is not changed but sigaction of SIGIO is changed.
int test_thread_inheritage() {
    int ret;
    printf("Run a parent process has pid = %d and ppid = %d\n", getpid(), getppid());

    signal(SIGIO, SIG_IGN);
    raise(SIGIO); // this should be ignored
    printf("SIGIO is ignored.\n");

#ifndef __GLIBC__
    struct __sigset_t sig_set;
#else
    sigset_t sig_set;
#endif
    sigemptyset (&sig_set);
    sigaddset(&sig_set, SIGABRT);
    sigprocmask(SIG_BLOCK, &sig_set, NULL);

    // child thread will change the sigmask and change sigaction of SIGIO to signal handler
    pthread_t tid;
    ret = pthread_create(&tid, NULL, thread_func, (void *)&sig_set);
    if (ret != 0) {
        THROW_ERROR("create child error");
    }

    pthread_join(tid, NULL);

#ifndef __GLIBC__
    // sigmask of father thread shouldn't be changed by child thread
    struct __sigset_t current_block_sigmask_master;
    sigprocmask(0, NULL, &current_block_sigmask_master);
    assert(current_block_sigmask_master.__bits[0] == sig_set.__bits[0]);
#endif

    // SIGIO sigaction should be changed by child thread
    printf("SIGIO should be handled:\n");
    raise(SIGIO); // this should be handled
    return 0;
}

// Parent process sets the sigmask of this child process to block SIGABORT by inheritage or posix_spawnattr_t.
int test_spawn_attribute_setsigmask() {
    int ret, child_pid, status;
    printf("Run a parent process has pid = %d and ppid = %d\n", getpid(), getppid());

    // construct child process args
    int child_argc = 3; // ./nauty_child -t sigmask
    char **child_argv = calloc(1, sizeof(char *) * (child_argc + 1));
    child_argv[0] = strdup("naughty_child");
    child_argv[1] = strdup("-t");
    child_argv[2] = strdup("sigmask");

    signal(SIGIO, sigio_handler);
    sigset_t sig_set;
    sigemptyset (&sig_set);
    sigaddset(&sig_set, SIGABRT);
    sigprocmask(SIG_BLOCK, &sig_set, NULL);
    // child process should inherit sigmask to block SIGABRT
    ret = posix_spawn(&child_pid, "/bin/naughty_child", NULL, NULL, child_argv, NULL);
    if (ret != 0) {
        THROW_ERROR("failed to spawn a child process\n");
    }
    printf("Spawn a new proces successfully (pid = %d)\n", child_pid);

    ret = waitpid(child_pid, &status, 0);
    if (ret < 0) {
        THROW_ERROR("failed to wait4 the child process\n");
    }
    printf("child process %d exit status = %d\n", child_pid, status);
    if (status != 0) {
        THROW_ERROR("child process exit with error");
    }

    // make parent process block SIGIO
    sigaddset(&sig_set, SIGIO);
    sigprocmask(SIG_BLOCK, &sig_set, NULL);

    posix_spawnattr_t attr;
    posix_spawnattr_init(&attr);

    posix_spawnattr_setflags(&attr, POSIX_SPAWN_SETSIGMASK);
    sigdelset(&sig_set, SIGIO); // Child process don't block SIGIO
    posix_spawnattr_setsigmask(&attr, &sig_set);

    ret = posix_spawn(&child_pid, "/bin/naughty_child", NULL, &attr, child_argv, NULL);
    if (ret != 0) {
        THROW_ERROR("failed to spawn a child process");
    }
    printf("Spawn a new proces successfully (pid = %d)\n", child_pid);

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

// Parent process sets the sigaction of SIGALRM and SIGILL to SIG_IGN and SIGIO to user-defined handler. Then use posix_spawn attribute to set
// SIGALRM to SIG_DEF for child process.
// Child process should inherit the ignore action of SIGILL and change SIGALRM and SIGIO sigaction to SIG_DEF.
int test_spawn_attribute_setsigdef() {
    int ret, child_pid, status;
    printf("Run a parent process has pid = %d and ppid = %d\n", getpid(), getppid());

    // construct child process args
    int child_argc = 3; // ./nauty_child -t sigdef
    char **child_argv = calloc(1, sizeof(char *) * (child_argc + 1));
    child_argv[0] = strdup("naughty_child");
    child_argv[1] = strdup("-t");
    child_argv[2] = strdup("sigdef");

    // parent process ignore SIGALRM and SIGILL and use user-defined signal handler for SIGIO
    signal(SIGIO, sigio_handler);
    signal(SIGILL, SIG_IGN);
    signal(SIGALRM, SIG_IGN);
    raise(SIGIO);
    raise(SIGILL);
    raise(SIGALRM);
    printf("parent process shouldn't handle SIGALRM and SIGILL\n");

    // use spawn attribute to set SIGALRM to default action
    sigset_t child_default_sigset;
    sigemptyset(&child_default_sigset);
    sigaddset(&child_default_sigset, SIGALRM);
    posix_spawnattr_t attr;
    posix_spawnattr_init(&attr);
    posix_spawnattr_setflags(&attr, POSIX_SPAWN_SETSIGDEF);
    posix_spawnattr_setsigdefault(&attr, &child_default_sigset);

    // child process should inherit sigaction to ignore SIGILL and set SIGIO and SIGALRM to SIG_DEF
    ret = posix_spawn(&child_pid, "/bin/naughty_child", NULL, &attr, child_argv, NULL);
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

    raise(SIGIO);
    raise(SIGILL);
    raise(SIGALRM);
    printf("parent process shouldn't handle SIGALRM and SIGILL\n");
    return 0;
}

// Create child process to pass naughty_child test by posix spawn attributes.
int test_multiple_spawn_attribute() {
    int ret, child_pid, status;
    printf("Run a parent process has pid = %d and ppid = %d\n", getpid(), getppid());

    // construct child process args
    int child_argc = 5; // ./naughty_child -t sigdef -t sigmask
    char **child_argv = calloc(1, sizeof(char *) * (child_argc + 1));
    child_argv[0] = strdup("naughty_child");
    child_argv[1] = strdup("-t");
    child_argv[2] =
        strdup("sigdef"); // child process SIGALRM and SIGIO have default action and SIGILL is ignored
    child_argv[3] = strdup("-t");
    child_argv[4] = strdup("sigmask"); // child process block SIGABORT

    posix_spawnattr_t attr;
    posix_spawnattr_init(&attr);
    posix_spawnattr_setflags(&attr, POSIX_SPAWN_SETSIGDEF | POSIX_SPAWN_SETSIGMASK);

    // use spawn attribute to set SIGALRM and SIGIO to default action
    sigset_t child_default_sigset;
    sigemptyset(&child_default_sigset);
    sigaddset(&child_default_sigset, SIGALRM);
    sigaddset(&child_default_sigset, SIGIO);
    posix_spawnattr_setsigdefault(&attr, &child_default_sigset);
    signal(SIGILL, SIG_IGN); // child will inherit this

    sigset_t child_sigmask;
    sigemptyset(&child_sigmask);
    sigaddset(&child_sigmask, SIGABRT);
    posix_spawnattr_setsigmask(&attr, &child_sigmask);

    // child process should inherit sigaction to ignore SIGABRT and set SIGIO to SIG_DEF
    ret = posix_spawn(&child_pid, "/bin/naughty_child", NULL, &attr, child_argv, NULL);
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

// ============================================================================
// Test suite
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_thread_inheritage),
    TEST_CASE(test_spawn_attribute_setsigmask),
    TEST_CASE(test_spawn_attribute_setsigdef),
    TEST_CASE(test_multiple_spawn_attribute),
};

int main() {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
