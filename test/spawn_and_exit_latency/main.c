#include <sys/syscall.h>
#include <sys/wait.h>
#include <sys/time.h>
#include <unistd.h>
#include <stdio.h>
#include <spawn.h>

#define NREPEATS 5000

int main(int argc, const char *argv[]) {
    struct timeval tv_start, tv_end;

    gettimeofday(&tv_start, NULL);
    for (unsigned long i = 0; i < NREPEATS; i++) {
        int child_pid, status;
        if (posix_spawn(&child_pid, "/bin/empty", NULL, NULL, NULL, NULL) < 0) {
            printf("ERROR: failed to spawn (# of repeats = %lu)\n", i);
            return -1;
        }
        if (wait4(-1, &status, 0, NULL) < 0) {
            printf("ERROR: failed to wait4 (# of repeats = %lu)\n", i);
            return -1;
        }
        if (status != 0) {
            printf("ERROR: child process exits with error\n");
            return -1;
        }
    }
    gettimeofday(&tv_end, NULL);

    suseconds_t total_us = (tv_end.tv_sec - tv_start.tv_sec) * 1000000UL +
                           + (tv_end.tv_usec - tv_start.tv_usec);
    suseconds_t latency = total_us / NREPEATS;
    printf("Latency of spawn/exit = %lu us\n", latency);
    return 0;
}
