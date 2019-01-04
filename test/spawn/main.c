#include <sys/syscall.h>
#include <unistd.h>
#include <stdio.h>

int main(void) {
    int ret, child_pid, status;
    printf("Run a parent process has pid = %d and ppid = %d\n", getpid(), getppid());

    ret = syscall(__NR_spawn, &child_pid, "getpid/bin.encrypted", NULL, NULL);
    if (ret < 0) {
        printf("ERROR: failed to spawn a child process\n");
        return -1;
    }
    printf("Spawn a new proces successfully (pid = %d)\n", child_pid);

    ret = syscall(__NR_wait4, -1, &status, 0);
    if (ret < 0) {
        printf("ERROR: failed to wait4 the child process\n");
        return -1;
    }
    printf("Child process exited with status = %d\n", status);

    return 0;
}
