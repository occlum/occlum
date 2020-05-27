#define _GNU_SOURCE
#include <sys/types.h>
#include <sys/stat.h>
#include <sys/mman.h>
#include <sys/wait.h>
#include <unistd.h>
#include <ucontext.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <spawn.h>
#include <assert.h>
#include <string.h>
#include <fcntl.h>
#include <signal.h>
#include "test.h"

// ============================================================================
// Helper macros
// ============================================================================


// ============================================================================
// Helper functions
// ============================================================================


// ============================================================================
// Test sigprocmask
// ============================================================================

// Add a new macro to compare two sigset. Returns 0 iff the two sigset are equal.
// Musl libc defines sigset_t to 16 bytes, but on x86 only the first 8 bytes are
// meaningful. So this comparison only takes the first 8 bytes into account.
#define sigcmpset(a, b) memcmp((a), (b), 8)

int test_sigprocmask() {
    int ret;
    sigset_t new, old;
    sigset_t expected_old;

    // Check sigmask == []
    if ((ret = sigprocmask(0, NULL, &old)) < 0) {
        THROW_ERROR("sigprocmask failed unexpectedly");
    }
    sigemptyset(&expected_old);
    if (sigcmpset(&old, &expected_old) != 0) {
        THROW_ERROR("unexpected old sigset");
    }

    // SIG_BLOCK: [] --> [SIGSEGV]
    sigemptyset(&new);
    sigaddset(&new, SIGSEGV);
    if ((ret = sigprocmask(SIG_BLOCK, &new, &old)) < 0) {
        THROW_ERROR("sigprocmask failed unexpectedly");
    }
    sigemptyset(&expected_old);
    if (sigcmpset(&old, &expected_old) != 0) {
        THROW_ERROR("unexpected old sigset");
    }

    // SIG_SETMASK: [SIGSEGV] --> [SIGIO]
    sigemptyset(&new);
    sigaddset(&new, SIGIO);
    if ((ret = sigprocmask(SIG_SETMASK, &new, &old)) < 0) {
        THROW_ERROR("sigprocmask failed unexpectedly");
    }
    sigemptyset(&expected_old);
    sigaddset(&expected_old, SIGSEGV);
    if (sigcmpset(&old, &expected_old) != 0) {
        THROW_ERROR("unexpected old sigset");
    }

    // SIG_UNBLOCK: [SIGIO] -> []
    if ((ret = sigprocmask(SIG_UNBLOCK, &new, &old)) < 0) {
        THROW_ERROR("sigprocmask failed unexpectedly");
    }
    sigemptyset(&expected_old);
    sigaddset(&expected_old, SIGIO);
    if (sigcmpset(&old, &expected_old) != 0) {
        THROW_ERROR("unexpected old sigset");
    }

    // Check sigmask == []
    if ((ret = sigprocmask(0, NULL, &old)) < 0) {
        THROW_ERROR("sigprocmask failed unexpectedly");
    }
    sigemptyset(&expected_old);
    if (sigcmpset(&old, &expected_old) != 0) {
        THROW_ERROR("unexpected old sigset");
    }

    return 0;
}

// ============================================================================
// Test raise syscall and user-registered signal handlers
// ============================================================================

#define MAX_RECURSION_LEVEL     3

static void handle_sigio(int num, siginfo_t *info, void *context) {
    static volatile int recursion_level = 0;
    printf("Hello from SIGIO signal handler (recursion_level = %d)!\n", recursion_level);

    recursion_level++;
    if (recursion_level <= MAX_RECURSION_LEVEL) {
        raise(SIGIO);
    }
    recursion_level--;
}

int test_raise() {
    struct sigaction new_action, old_action;
    new_action.sa_sigaction = handle_sigio;
    new_action.sa_flags = SA_SIGINFO | SA_NODEFER;
    if (sigaction(SIGIO, &new_action, &old_action) < 0) {
        THROW_ERROR("registering new signal handler failed");
    }
    if (old_action.sa_handler != SIG_DFL) {
        THROW_ERROR("unexpected old sig handler");
    }

    raise(SIGIO);

    if (sigaction(SIGIO, &old_action, NULL) < 0) {
        THROW_ERROR("restoring old signal handler failed");
    }
    return 0;
}

// ============================================================================
// Test abort, which uses SIGABRT behind the scene
// ============================================================================

int test_abort() {
    pid_t child_pid;
    char *child_argv[] = {"signal", "aborted_child", NULL};
    int ret;
    int status;

    // Repeat multiple times to check that the resources of the killed child
    // processes are indeed freed by the LibOS
    for (int i = 0; i < 3; i++) {
        ret = posix_spawn(&child_pid, "/bin/signal", NULL, NULL, child_argv, NULL);
        if (ret < 0) {
            THROW_ERROR("failed to spawn a child process\n");
        }

        ret = wait4(-1, &status, 0, NULL);
        if (ret < 0) {
            THROW_ERROR("failed to wait4 the child process\n");
        }
        if (!WIFSIGNALED(status) || WTERMSIG(status) != SIGABRT) {
            THROW_ERROR("child process is expected to be killed by SIGILL\n");
        }
    }
    return 0;
}

static int aborted_child() {
    while (1) {
        abort();
    }
    return 0;
}

// ============================================================================
// Test kill by sending SIGKILL to another process
// ============================================================================

int test_kill() {
    pid_t child_pid;
    char *child_argv[] = {"signal", "killed_child", NULL};
    int ret;
    int status;

    // Repeat multiple times to check that the resources of the killed child
    // processes are indeed freed by the LibOS
    for (int i = 0; i < 3; i++) {
        ret = posix_spawn(&child_pid, "/bin/signal", NULL, NULL, child_argv, NULL);
        if (ret < 0) {
            THROW_ERROR("failed to spawn a child process\n");
        }

        kill(child_pid, SIGKILL);

        ret = wait4(-1, &status, 0, NULL);
        if (ret < 0) {
            THROW_ERROR("failed to wait4 the child process\n");
        }
        if (!WIFSIGNALED(status) || WTERMSIG(status) != SIGKILL) {
            THROW_ERROR("child process is expected to be killed by SIGILL\n");
        }
    }
    return 0;
}

// TODO: remove the use of getpid when we can deliver signals through interrupt
static int killed_child() {
    while (1) {
        getpid();
    }
    return 0;
}

// ============================================================================
// Test catching and handling hardware exception
// ============================================================================

static void handle_sigfpe(int num, siginfo_t *info, void *_context) {
    printf("SIGFPE Caught\n");
    assert(num == SIGFPE);
    assert(info->si_signo == SIGFPE);

    ucontext_t *ucontext = _context;
    mcontext_t *mcontext = &ucontext->uc_mcontext;
    // The faulty instruction should be `idiv %esi` (f7 fe)
    mcontext->gregs[REG_RIP] += 2;

    return;
}

// Note: this function is fragile in the sense that compiler may not always
// emit the instruction pattern that triggers divide-by-zero as we expect.
// TODO: rewrite this in assembly
int div_maybe_zero(int x, int y) {
    return x / y;
}

int test_catch_fault() {
#ifdef SGX_MODE_SIM
    printf("WARNING: Skip this test case as we do not support "
           "capturing hardware exception in SGX simulation mode\n");
    return 0;
#else
    // Set up a signal handler that handles divide-by-zero exception
    struct sigaction new_action, old_action;
    new_action.sa_sigaction = handle_sigfpe;
    new_action.sa_flags = SA_SIGINFO;
    if (sigaction(SIGFPE, &new_action, &old_action) < 0) {
        THROW_ERROR("registering new signal handler failed");
    }
    if (old_action.sa_handler != SIG_DFL) {
        THROW_ERROR("unexpected old sig handler");
    }

    // Trigger divide-by-zero exception
    int a = 1;
    int b = 0;
    // Use volatile to prevent compiler optimization
    volatile int c;
    c = div_maybe_zero(a, b);

    printf("Signal handler successfully jumped over the divide-by-zero instruction\n");

    if (sigaction(SIGFPE, &old_action, NULL) < 0) {
        THROW_ERROR("restoring old signal handler failed");
    }
    return 0;
#endif /* SGX_MODE_SIM */
}

// ============================================================================
// Test handle signal on alternate signal stack
// ============================================================================

#define MAX_ALTSTACK_RECURSION_LEVEL    2

stack_t g_old_ss;

static void handle_sigpipe(int num, siginfo_t *info, void *context) {
    static volatile int recursion_level = 0;
    printf("Hello from SIGPIPE signal handler on the alternate signal stack (recursion_level = %d)\n",
           recursion_level);

    // save old_ss to check if we are on stack
    stack_t old_ss;
    sigaltstack(NULL, &old_ss);
    g_old_ss = old_ss;

    recursion_level++;
    if (recursion_level <= MAX_ALTSTACK_RECURSION_LEVEL) {
        raise(SIGPIPE);
    }
    recursion_level--;
}

int test_sigaltstack() {
    static char stack[SIGSTKSZ];
    stack_t expected_ss = {
        .ss_size = SIGSTKSZ,
        .ss_sp = stack,
        .ss_flags = 0,
    };
    if (sigaltstack(&expected_ss, NULL) < 0) {
        THROW_ERROR("failed to call sigaltstack");
    }
    stack_t actual_ss;
    if (sigaltstack(NULL, &actual_ss) < 0) {
        THROW_ERROR("failed to call sigaltstack");
    }
    if (actual_ss.ss_size != expected_ss.ss_size
            || actual_ss.ss_sp != expected_ss.ss_sp
            || actual_ss.ss_flags != expected_ss.ss_flags) {
        THROW_ERROR("failed to check the signal stack after set");
    }

    struct sigaction new_action, old_action;
    new_action.sa_sigaction = handle_sigpipe;
    new_action.sa_flags = SA_SIGINFO | SA_NODEFER | SA_ONSTACK;
    if (sigaction(SIGPIPE, &new_action, &old_action) < 0) {
        THROW_ERROR("registering new signal handler failed");
    }
    if (old_action.sa_handler != SIG_DFL) {
        THROW_ERROR("unexpected old sig handler");
    }

    raise(SIGPIPE);
    if (g_old_ss.ss_flags != SS_ONSTACK) {
        THROW_ERROR("check stack flags failed");
    }

    if (sigaction(SIGPIPE, &old_action, NULL) < 0) {
        THROW_ERROR("restoring old signal handler failed");
    }
    return 0;
}

// ============================================================================
// Test SIGCHLD signal
// ============================================================================
int sigchld = 0;

void proc_exit() {
    sigchld = 1;
}

int test_sigchld() {
    signal(SIGCHLD, proc_exit);

    int ret, child_pid;
    printf("Run a parent process has pid = %d and ppid = %d\n", getpid(), getppid());

    ret = posix_spawn(&child_pid, "/bin/getpid", NULL, NULL, NULL, NULL);
    if (ret < 0) {
        printf("ERROR: failed to spawn a child process\n");
        return -1;
    }
    printf("Spawn a new proces successfully (pid = %d)\n", child_pid);

    wait(NULL);
    if (sigchld == 0) { THROW_ERROR("Did not receive SIGCHLD"); }

    return 0;
}

// ============================================================================
// Test suite main
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_sigprocmask),
    TEST_CASE(test_raise),
    TEST_CASE(test_abort),
    TEST_CASE(test_kill),
    TEST_CASE(test_catch_fault),
    TEST_CASE(test_sigaltstack),
    TEST_CASE(test_sigchld),
};

int main(int argc, const char *argv[]) {
    if (argc > 1) {
        const char *cmd = argv[1];
        if (strcmp(cmd, "aborted_child") == 0) {
            return aborted_child();
        } else if (strcmp(cmd, "killed_child") == 0) {
            return killed_child();
        } else {
            fprintf(stderr, "ERROR: unknown command: %s\n", cmd);
            return EXIT_FAILURE;
        }
    }

    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
