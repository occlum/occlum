#include <sys/syscall.h>
#include <sys/wait.h>
#include <unistd.h>
#include <stdio.h>
#include <spawn.h>

int main(int argc, const char *argv[]) {
    int ret, child_pid, status;
    printf("Run a parent process has pid = %d and ppid = %d\n", getpid(), getppid());

    ret = posix_spawn(&child_pid, "/bin/getpid", NULL, NULL, NULL, NULL);
    if (ret < 0) {
        printf("ERROR: failed to spawn a child process\n");
        return -1;
    }
    printf("Spawn a new proces successfully (pid = %d)\n", child_pid);

    ret = wait4(-1, &status, 0, NULL);
    if (ret < 0) {
        printf("ERROR: failed to wait4 the child process\n");
        return -1;
    }
    printf("Child process exited with status = %d\n", status);

    return 0;
}
