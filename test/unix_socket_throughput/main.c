#include <sys/syscall.h>
#include <sys/time.h>
#include <sys/wait.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>
#include <stdlib.h>
#include <stdio.h>
#include <spawn.h>
#include <string.h>

#define KB              (1024UL)
#define MB              (1024UL * 1024UL)
#define GB              (1024UL * 1024UL * 1024UL)

#define TOTAL_BYTES     (2 * GB)
#define BUF_SIZE        (128 * KB)

#define MIN(x, y)       ((x) <= (y) ? (x) : (y))

const char SOCK_PATH[] = "echo_socket";

int create_server_socket() {
    int fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (fd == -1) {
        printf("ERROR: failed to create a unix socket\n");
        return -1;
    }

    struct sockaddr_un local;
    local.sun_family = AF_UNIX;
    strcpy(local.sun_path, SOCK_PATH);
    unlink(local.sun_path);
    socklen_t len = strlen(local.sun_path) + sizeof(local.sun_family);

    if (bind(fd, (struct sockaddr *)&local, len) == -1) {
        printf("ERROR: failed to bind\n");
        return -1;
    }

    if (listen(fd, 5) == -1) {
        printf("ERROR: failed to listen\n");
        return -1;
    }
    return fd;
}

int create_client_socket() {
    int fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (fd == -1) {
        printf("ERROR: failed to create a unix socket\n");
        return -1;
    }

    struct sockaddr_un remote;
    remote.sun_family = AF_UNIX;
    strcpy(remote.sun_path, SOCK_PATH);
    socklen_t len = strlen(remote.sun_path) + sizeof(remote.sun_family);

    if (connect(fd, (struct sockaddr *)&remote, len) == -1) {
        printf("ERROR: failed to connect\n");
        return -1;
    }
    return fd;
}

int main(int argc, const char *argv[]) {
    size_t buf_size, total_bytes;
    if (argc >= 2) {
        buf_size = atol(argv[1]);
    } else {
        buf_size = BUF_SIZE;
    }
    if (argc >= 3) {
        total_bytes = atol(argv[2]);
    } else {
        // BUG: throughput fall down when buf_size > 65536
        total_bytes = buf_size > 65536 ? buf_size << 15 : buf_size << 21;
    }
    printf("buf_size = 0x%zx\n", buf_size);
    printf("total_bytes = 0x%zx\n", total_bytes);

    int listen_fd = create_server_socket();
    if (listen_fd == -1) {
        printf("ERROR: failed to create server socket\n");
        return -1;
    }

    int socket_rd_fd = create_client_socket();
    if (socket_rd_fd == -1) {
        printf("ERROR: failed to create client socket\n");
        return -1;
    }

    struct sockaddr_un remote;
    socklen_t len = sizeof(remote);
    int socket_wr_fd = accept(listen_fd, (struct sockaddr *)&remote, &len);
    if (socket_wr_fd == -1) {
        printf("ERROR: failed to accept socket\n");
        return -1;
    }

    // The following is same as 'pipe_throughput'

    // Spawn a child process that reads from the pipe
    posix_spawn_file_actions_t file_actions;
    posix_spawn_file_actions_init(&file_actions);
    posix_spawn_file_actions_adddup2(&file_actions, socket_rd_fd, STDIN_FILENO);
    posix_spawn_file_actions_addclose(&file_actions, socket_wr_fd);

    int child_pid;
    extern char **environ;
    char *new_argv[] = {"/bin/data_sink", NULL};
    if (posix_spawn(&child_pid, "/bin/data_sink", &file_actions,
                    NULL, new_argv, environ) < 0) {
        printf("ERROR: failed to spawn a child process\n");
        return -1;
    }
    close(socket_rd_fd);

    // Start the timer
    struct timeval tv_start, tv_end;
    gettimeofday(&tv_start, NULL);

    // Tell the reader how many data are to be transfered
    size_t remain_bytes = total_bytes;
    if (write(socket_wr_fd, &remain_bytes, sizeof(remain_bytes)) != sizeof(remain_bytes)) {
        printf("ERROR: failed to write to pipe\n");
        return -1;
    }

    // Tell the reader the buffer size that it should use
    if (write(socket_wr_fd, &buf_size, sizeof(buf_size)) != sizeof(buf_size)) {
        printf("ERROR: failed to write to pipe\n");
        return -1;
    }

    // Write a specified amount of data in a buffer of specified size
    char buf[BUF_SIZE] = {0};
    while (remain_bytes > 0) {
        size_t len = MIN(buf_size, remain_bytes);
        if ((len = write(socket_wr_fd, &buf, len)) < 0) {
            printf("ERROR: failed to write to pipe\n");
            return -1;
        }
        remain_bytes -= len;
    }

    // Wait for the child process to read all data and exit
    int status = 0;
    if (wait4(child_pid, &status, 0, NULL) < 0) {
        printf("ERROR: failed to wait4 the child process\n");
        return -1;
    }

    // Stop the timer
    gettimeofday(&tv_end, NULL);

    // Calculate the throughput
    double total_s = (tv_end.tv_sec - tv_start.tv_sec)
                     + (double)(tv_end.tv_usec - tv_start.tv_usec) / 1000000;
    if (total_s < 1.0) {
        printf("WARNING: run long enough to get meaningful results\n");
        if (total_s == 0) { return 0; }
    }
    double total_mb = (double)total_bytes / MB;
    double throughput = total_mb / total_s;
    printf("Throughput of unix socket is %.2f MB/s\n", throughput);
    return 0;
}
