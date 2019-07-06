#include <sys/syscall.h>
#include <sys/wait.h>
#include <unistd.h>
#include <stdlib.h>
#include <stdio.h>
#include <spawn.h>
#include <string.h>

int main(int argc, const char* argv[]) {
    // XXX: this is a hack! remove this in the future
    void* ptr = malloc(64);
    free(ptr);

    int pipe_fds[2];
    if (pipe(pipe_fds) < 0) {
        printf("ERROR: failed to create a pipe\n");
        return -1;
    }
    int pipe_rd_fd = pipe_fds[0];
    int pipe_wr_fd = pipe_fds[1];

    posix_spawn_file_actions_t file_actions;
    posix_spawn_file_actions_init(&file_actions);
    posix_spawn_file_actions_adddup2(&file_actions, pipe_wr_fd, STDOUT_FILENO);
    posix_spawn_file_actions_addclose(&file_actions, pipe_rd_fd);

    const char* msg = "Echo!\n";
    const char* child_prog = "hello_world";
    const char* child_argv[3] = { child_prog, msg, NULL };
    int child_pid;
    if (posix_spawn(&child_pid, child_prog, &file_actions,
            NULL, (char*const *)child_argv, NULL) < 0) {
        printf("ERROR: failed to spawn a child process\n");
        return -1;
    }
    close(pipe_wr_fd);

    const char* expected_str = msg;
    size_t expected_len = strlen(expected_str);
    char actual_str[32] = {0};
    ssize_t actual_len;
    do {
        actual_len = read(pipe_rd_fd, actual_str, sizeof(actual_str) - 1);
    } while (actual_len == 0);
    if (strncmp(expected_str, actual_str, expected_len) != 0) {
        printf("ERROR: received string is not as expected\n");
        return -1;
    }

    int status = 0;
    if (wait4(child_pid, &status, 0, NULL) < 0) {
        printf("ERROR: failed to wait4 the child process\n");
        return -1;
    }
    return 0;
}
