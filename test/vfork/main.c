#define _GNU_SOURCE
#include <stdio.h>
#include <fcntl.h>
#include <stdlib.h>
#include <sys/wait.h>
#include <pthread.h>
#include "test.h"

// Note: This test intends to test the case that child process directly calls _exit()
// after vfork. "exit", "_exit" and returning from main function are different.
// And here the exit function must be "_exit" to prevent undefined bevaviour.
int test_vfork_exit_and_wait() {
    int status = 0;
    pid_t child_pid = vfork();
    if (child_pid == 0) {
        _exit(0);
    } else {
        printf ("Comming back to parent process from child with pid = %d\n", child_pid);

        // vfork again
        pid_t child_pid_2 = vfork();
        if (child_pid_2 == 0) {
            _exit(1);
        } else {
            printf ("Comming back to parent process from child with pid = %d\n", child_pid_2);
            int ret = waitpid(child_pid, &status, WUNTRACED);
            if (ret != child_pid  || !WIFEXITED(status) || WEXITSTATUS(status) != 0) {
                THROW_ERROR("wait child status error");
            }
            ret = waitpid(child_pid_2, &status, WUNTRACED);
            if (ret != child_pid_2  || !WIFEXITED(status) || WEXITSTATUS(status) != 1) {
                THROW_ERROR("wait child status error");
            }
        }
    }
    return 0;
}

int test_multiple_vfork_execve() {
    char **child_argv = calloc(1, sizeof(char *) * 2); // "hello_world", NULL
    child_argv[0] = strdup("naughty_child");
    for (int i = 0; i < 3; i++ ) {
        pid_t child_pid = vfork();
        if (child_pid == 0) {
            int ret = execve("/bin/naughty_child", child_argv, NULL);
            if (ret != 0) {
                printf("child process execve error");
            }
            _exit(1);
        } else {
            printf ("Comming back to parent process from child with pid = %d\n", child_pid);
            int ret = waitpid(child_pid, 0, 0);
            if (ret != child_pid) {
                THROW_ERROR("wait child error, child pid = %d\n", child_pid);
            }
        }
    }
    return 0;
}

// Create a pipe between parent and child and check file status.
int test_vfork_isolate_file_table() {
    int pipe_fds[2];
    if (pipe(pipe_fds) < 0) {
        THROW_ERROR("failed to create a pipe");
    }

    pid_t child_pid = vfork();
    if (child_pid == 0) {
        close(pipe_fds[1]); // close write end
        char **child_argv = calloc(1,
                                   sizeof(char *) * (5 + 1)); // naughty_child -t vfork reader_fd writer_fd
        child_argv[0] = "naughty_child";
        child_argv[1] = "-t";
        child_argv[2] = "vfork";
        if (asprintf(&child_argv[3], "%d", pipe_fds[0]) < 0 ||
                asprintf(&child_argv[4], "%d", pipe_fds[1]) < 0) {
            THROW_ERROR("failed to asprintf");
        }

        int ret = execve("/bin/naughty_child", child_argv, NULL);
        if (ret != 0) {
            printf("child process execve error\n");
        }
        _exit(1);
    } else {
        printf ("Comming back to parent process from child with pid = %d\n", child_pid);
        if (close(pipe_fds[0]) < 0) { // close read end
            printf("close pipe reader error\n");
            goto parent_exit;
        }
        char *greetings = "Hello from parent\n";
        if (write(pipe_fds[1], greetings, strlen(greetings) + 1) < 0) {
            printf("parent write pipe error\n");
            goto parent_exit;
        }
        int ret = waitpid(child_pid, 0, 0);
        if (ret != child_pid) {
            THROW_ERROR("wait child error, child pid = %d\n", child_pid);
        }
    }

    return 0;

parent_exit:
    kill(child_pid, SIGKILL);
    exit(1);
}

volatile static int test_stop_child_flag = 0;

static void *child_thread_routine(void *_arg) {
    printf("Child thread starts\n");
    test_stop_child_flag = 1;

    struct timespec t1, t2;
    if (clock_gettime(CLOCK_REALTIME, &t1)) {
        return (void *) -1;
    }

    int i = 0;
    while (1) {
        i++;
        int ret = sleep(1);
        if (ret == 0 || i >= 10) {
            break;
        } else if (errno == EINTR) {
            // Interrupted, sleep again
            continue;
        }
    }

    if (clock_gettime(CLOCK_REALTIME, &t2)) {
        return (void *) -1;
    }

    // Parent thread vfork and will stop this thread for several seconds
    if (t2.tv_sec - t1.tv_sec <= 1) {
        printf("the thread is not stopped");
        exit(-1);
    }

    printf("child thread exits\n");
    return NULL;
}

// Test the behavior that when vfork is called, the parent process' other child threads are forced to stopped.
//
// This test case has different behaviors for Linux and Occlum
// This limitation is recorded in src/libos/src/process/do_vfork.rs
int test_vfork_stop_child_thread() {
    pthread_t child_thread;
    pid_t child_pid;
    struct timespec ts;
    ts.tv_sec = 3;
    ts.tv_nsec = 0;
    if (pthread_create(&child_thread, NULL, child_thread_routine, NULL) < 0) {
        THROW_ERROR("pthread_create failed\n");
    }

    // Wait for child thread to start
    while (test_stop_child_flag == 0);

    child_pid = vfork();
    if (child_pid == 0) {
        printf("child process created\n");
        char **child_argv = calloc(1, sizeof(char *) * 2);
        child_argv[0] = "getpid";

        // Wait for a few seconds
        while (1) {
            int ret = nanosleep(&ts, &ts);
            if (ret == 0) {
                break;
            }
            if (ret < 0 && errno != EINTR) {
                THROW_ERROR("nanosleep failed");
            }
        }

        printf("child process exec\n");
        int ret = execve("/bin/getpid", child_argv, NULL);
        if (ret != 0) {
            printf("child process execve error\n");
        }
        _exit(1);
    } else {
        printf("return to parent\n");

        pthread_join(child_thread, NULL);
    }

    return 0;
}

#define NUM_THREADS 20
volatile static int test_main_thread_is_ready = 0;

void *child_thread(void *arg) {
    int *number = (int *)arg;

    int repeat = 10;
    if (*number == 3) {
        printf("child thread %d do vfork\n", *number);
        fflush(stdout);
        // This thread will continually vfork and exit
        int i = repeat;
        while (i--) {
            // wait for main thread to be ready for vfork
            while (test_main_thread_is_ready == 0);
            pid_t pid = vfork();
            if (pid == 0) {
                // Child process
                sleep(1);
                _exit(0);
            } else if (pid > 0) {
                // Parent process
                waitpid(pid, NULL, 0);
                printf("child vfork i = %d\n", i);
            } else {
                perror("vfork");
                exit(EXIT_FAILURE);
            }
        }

        return NULL;
    }

    // Other threads do their own work
    for (int i = 5; i < repeat; ++i) {
        printf("Thread %ld doing its work i = %d.\n", pthread_self(), i);
        fflush(stdout);
        sleep(1);
    }

    return NULL;
}

// Test multiple threads of the same process do vfork simultaneously and shouldn't force stop each other to make the process hang.
int test_vfork_multiple_threads() {
    pthread_t threads[NUM_THREADS];
    int ret;
    int test[NUM_THREADS] = {0};

    // Create NUM_THREADS threads
    for (int i = 0; i < NUM_THREADS; ++i) {
        test[i] = i;
        ret = pthread_create(&threads[i], NULL, child_thread, &test[i]);
        if (ret != 0) {
            perror("pthread_create");
            return EXIT_FAILURE;
        }
    }
    printf("create child threads done\n");
    fflush(stdout);

    test_main_thread_is_ready = 1;
    // Main thread does a vfork and exec hello_world
    pid_t pid = vfork();
    if (pid == 0) {
        // Child process
        sleep(1);
        char *args[] = { "/bin/getpid", NULL };
        execv(args[0], args);
        perror("execv");
        _exit(EXIT_FAILURE); // Exit if exec fails
    } else if (pid > 0) {
        // Parent process waits for the child to complete
        waitpid(pid, NULL, 0);
    } else {
        perror("vfork");
        return EXIT_FAILURE;
    }

    // Join the threads
    for (int i = 0; i < NUM_THREADS; ++i) {
        pthread_join(threads[i], NULL);
    }

    return 0;
}

static test_case_t test_cases[] = {
    TEST_CASE(test_vfork_exit_and_wait),
    TEST_CASE(test_multiple_vfork_execve),
    TEST_CASE(test_vfork_isolate_file_table),
    TEST_CASE(test_vfork_stop_child_thread),
    TEST_CASE(test_vfork_multiple_threads),
};

int main() {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
