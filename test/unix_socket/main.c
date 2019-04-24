#include <sys/syscall.h>
#include <sys/wait.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>
#include <stdlib.h>
#include <stdio.h>
#include <spawn.h>
#include <string.h>

const char SOCK_PATH[] = "echo_socket";

int create_server_socket() {
	int fd = socket(AF_UNIX, SOCK_STREAM, 0);
	if (fd == -1) {
		printf("ERROR: failed to create a unix socket");
		return -1;
	}

	struct sockaddr_un local;
	local.sun_family = AF_UNIX;
	strcpy(local.sun_path, SOCK_PATH);
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
		printf("ERROR: failed to create a unix socket");
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

int main(int argc, const char* argv[]) {
	int listen_fd = create_server_socket();
	if (listen_fd == -1) {
		printf("ERROR: failed to create server socket");
		return -1;
	}

	int socket_rd_fd = create_client_socket();
	if (socket_rd_fd == -1) {
		printf("ERROR: failed to create client socket");
		return -1;
	}

	struct sockaddr_un remote;
	socklen_t len = sizeof(remote);
	int socket_wr_fd = accept(listen_fd, (struct sockaddr *)&remote, &len);
	if (socket_wr_fd == -1) {
		printf("ERROR: failed to accept socket");
		return -1;
	}

	// The following is same as 'pipe'

    posix_spawn_file_actions_t file_actions;
    posix_spawn_file_actions_init(&file_actions);
    posix_spawn_file_actions_adddup2(&file_actions, socket_wr_fd, STDOUT_FILENO);
    posix_spawn_file_actions_addclose(&file_actions, socket_rd_fd);

    const char* msg = "Echo!\n";
    const char* child_prog = "hello_world";
    const char* child_argv[3] = { child_prog, msg, NULL };
    int child_pid;
    if (posix_spawn(&child_pid, child_prog, &file_actions,
            NULL, child_argv, NULL) < 0) {
        printf("ERROR: failed to spawn a child process\n");
        return -1;
    }
    close(socket_wr_fd);

    const char* expected_str = msg;
    size_t expected_len = strlen(expected_str);
    char actual_str[32] = {0};
    ssize_t actual_len;
    do {
        actual_len = read(socket_rd_fd, actual_str, sizeof(actual_str) - 1);
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
