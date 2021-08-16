#define _GNU_SOURCE
#include <sys/types.h>
#include <unistd.h>
#include <stdio.h>
#include <sys/syscall.h>

int main(int argc, const char *argv[]) {
    printf("Run a new process with pid = %d, ppid = %d, pgid = %d\n", getpid(), getppid(),
           getpgid(0));
    printf("tid = %ld\n", syscall(SYS_gettid));
    return 0;
}
