#include <sys/types.h>
#include <unistd.h>
#include <stdio.h>

int main(int argc, const char *argv[]) {
    printf("Run a new process with pid = %d and ppid = %d\n", getpid(), getppid());
    return 0;
}
