#include <sys/types.h>
#include <unistd.h>
#include <stdio.h>

int main(void) {
    printf("Run a new process with pid = %d and ppid = %d\n", getpid(), getppid());
    return 0;
}
