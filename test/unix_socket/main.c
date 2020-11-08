#include <sys/syscall.h>
#include <sys/wait.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <poll.h>
#include <unistd.h>
#include <stdlib.h>
#include <stdio.h>
#include <spawn.h>
#include <string.h>
#include <spawn.h>

#include "test.h"

#define ECHO_MSG "echo msg for unix_socket test"

int create_connected_sockets(int *sockets, char *sock_path) {
    int listen_fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (listen_fd == -1) {
        THROW_ERROR("failed to create a unix socket");
    }

    struct sockaddr_un addr;
    memset(&addr, 0, sizeof(struct sockaddr_un)); //Clear structure
    addr.sun_family = AF_UNIX;
    strcpy(addr.sun_path, sock_path);
    socklen_t addr_len = strlen(addr.sun_path) + sizeof(addr.sun_family);
    if (bind(listen_fd, (struct sockaddr *)&addr, addr_len) == -1) {
        close(listen_fd);
        THROW_ERROR("failed to bind");
    }

    if (listen(listen_fd, 5) == -1) {
        close(listen_fd);
        THROW_ERROR("failed to listen");
    }

    int client_fd = socket(AF_UNIX, SOCK_STREAM, PF_UNIX);
    if (client_fd == -1) {
        close(listen_fd);
        THROW_ERROR("failed to create a unix socket");
    }

    if (connect(client_fd, (struct sockaddr *)&addr, addr_len) == -1) {
        close(listen_fd);
        close(client_fd);
        THROW_ERROR("failed to connect");
    }

    int accepted_fd = accept(listen_fd, (struct sockaddr *)&addr, &addr_len);
    if (accepted_fd == -1) {
        close(listen_fd);
        close(client_fd);
        THROW_ERROR("failed to accept socket");
    }

    sockets[0] = client_fd;
    sockets[1] = accepted_fd;
    close(listen_fd);
    return 0;
}

int create_connceted_sockets_default(int *sockets) {
    return create_connected_sockets(sockets, "unix_socket_default_path");
}

int verify_child_echo(int *connected_sockets) {
    const char *child_prog = "/bin/hello_world";
    const char *child_argv[3] = { child_prog, ECHO_MSG, NULL };
    int child_pid;
    posix_spawn_file_actions_t file_actions;

    posix_spawn_file_actions_init(&file_actions);
    posix_spawn_file_actions_adddup2(&file_actions, connected_sockets[0], STDOUT_FILENO);
    posix_spawn_file_actions_addclose(&file_actions, connected_sockets[1]);

    if (posix_spawn(&child_pid, child_prog, &file_actions,
                    NULL, (char *const *)child_argv, NULL) < 0) {
        THROW_ERROR("failed to spawn a child process");
    }

    char actual_str[32] = {0};
    ssize_t actual_len;
    //TODO: implement blocking read
    do {
        actual_len = read(connected_sockets[1], actual_str, 32);
    } while (actual_len == 0);
    if (strncmp(actual_str, ECHO_MSG, sizeof(ECHO_MSG) - 1) != 0) {
        printf("data read is :%s\n", actual_str);
        THROW_ERROR("received string is not as expected");
    }

    int status = 0;
    if (wait4(child_pid, &status, 0, NULL) < 0) {
        THROW_ERROR("failed to wait4 the child process");
    }

    return 0;
}

int verify_connection(int src_sock, int dest_sock) {
    char buf[1024];
    int i;
    for (i = 0; i < 100; i++) {
        if (i % 2 == 0) {
            if (write(src_sock, ECHO_MSG, sizeof(ECHO_MSG)) < 0) {
                THROW_ERROR("writing server message");
            }
        } else {
            if (sendto(src_sock, ECHO_MSG, sizeof(ECHO_MSG), 0, NULL, 0) < 0) {
                THROW_ERROR("sendto server message");
            }
        }

        if (read(dest_sock, buf, 1024) < 0) {
            THROW_ERROR("reading server message");
        }

        if (strncmp(buf, ECHO_MSG, sizeof(ECHO_MSG)) != 0) {
            THROW_ERROR("msg received mismatch");
        }
    }
    return 0;
}

//this value should not be too large as one pair consumes 2MB memory
#define PAIR_NUM 15

int test_multiple_socketpairs() {
    int sockets[PAIR_NUM][2];
    int i;
    int ret = 0;

    for (i = 0; i < PAIR_NUM; i++) {
        if (socketpair(AF_UNIX, SOCK_STREAM, 0, sockets[i]) < 0) {
            THROW_ERROR("opening stream socket pair");
        }

        if (verify_connection(sockets[i][0], sockets[i][1]) < 0) {
            ret = -1;
            goto cleanup;
        }

        if (verify_connection(sockets[i][1], sockets[i][0]) < 0) {
            ret = -1;
            goto cleanup;
        }
    }
    i--;
cleanup:
    for (; i >= 0; i--) {
        close(sockets[i][0]);
        close(sockets[i][1]);
    }
    return ret;
}

int socketpair_default(int *sockets) {
    return socketpair(AF_UNIX, SOCK_STREAM, 0, sockets);
}

typedef int(*create_connection_func_t)(int *);
int test_connected_sockets_inter_process(create_connection_func_t fn) {
    int ret = 0;
    int sockets[2];
    if (fn(sockets) < 0) {
        return -1;
    }

    ret = verify_child_echo(sockets);

    close(sockets[0]);
    close(sockets[1]);
    return ret;
}

int test_unix_socket_inter_process() {
    return test_connected_sockets_inter_process(socketpair_default);
}

int test_socketpair_inter_process() {
    return test_connected_sockets_inter_process(create_connceted_sockets_default);
}

int test_poll() {
    int socks[2];
    if (socketpair(AF_UNIX, SOCK_STREAM, 0, socks) < 0) {
        THROW_ERROR("socketpair failed");
    }

    write(socks[0], "not today\n", 10);

    struct pollfd polls[] = {
        { .fd = socks[1], .events = POLLIN },
        { .fd = socks[0], .events = POLLOUT },
    };

    int ret = poll(polls, 2, 5000);
    if (ret <= 0) { THROW_ERROR("poll error"); }
    if ((polls[0].revents & POLLOUT) && (polls[1].revents && POLLIN) == 0) {
        THROW_ERROR("wrong return events");
    }
    return 0;
}

static test_case_t test_cases[] = {
    TEST_CASE(test_unix_socket_inter_process),
    TEST_CASE(test_socketpair_inter_process),
    TEST_CASE(test_multiple_socketpairs),
    // TODO: recover the test after the unix sockets are rewritten by using
    // the new event subsystem
    //TEST_CASE(test_poll),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
