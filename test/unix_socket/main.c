#include <sys/syscall.h>
#include <sys/wait.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <sys/ioctl.h>
#include <poll.h>
#include <unistd.h>
#include <stdlib.h>
#include <stdio.h>
#include <spawn.h>
#include <string.h>

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
    socklen_t addr_len = strlen(addr.sun_path) + sizeof(addr.sun_family) + 1;
    if (bind(listen_fd, (struct sockaddr *)&addr, addr_len) == -1) {
        close(listen_fd);
        THROW_ERROR("failed to bind");
    }

    if (listen(listen_fd, 5) == -1) {
        close(listen_fd);
        THROW_ERROR("failed to listen");
    }

    int client_fd = socket(AF_UNIX, SOCK_STREAM, 0);
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

int create_connected_sockets_then_rename(int *sockets) {
    char *socket_original_path = "/tmp/socket_tmp";
    char *socket_ready_path = "/tmp/.socket_tmp";
    int listen_fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (listen_fd == -1) {
        THROW_ERROR("failed to create a unix socket");
    }

    struct sockaddr_un addr;
    memset(&addr, 0, sizeof(struct sockaddr_un)); //Clear structure
    addr.sun_family = AF_UNIX;
    strcpy(addr.sun_path, socket_original_path);

    // About addr_len (from man page):
    // a UNIX domain socket can be bound to a null-terminated
    // filesystem pathname using bind(2).  When the address of
    // a pathname socket is returned (by one of the system
    // calls noted above), its length is:
    //  offsetof(struct sockaddr_un, sun_path) + strlen(sun_path) + 1
    socklen_t addr_len = strlen(addr.sun_path) + sizeof(addr.sun_family) + 1;
    if (bind(listen_fd, (struct sockaddr *)&addr, addr_len) == -1) {
        close(listen_fd);
        THROW_ERROR("failed to bind");
    }

    if (listen(listen_fd, 5) == -1) {
        close(listen_fd);
        THROW_ERROR("failed to listen");
    }

    // rename to new path
    unlink(socket_ready_path);
    if (rename(socket_original_path, socket_ready_path) < 0) {
        THROW_ERROR("failed to rename");
    }

    int client_fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (client_fd == -1) {
        close(listen_fd);
        THROW_ERROR("failed to create a unix socket");
    }

    struct sockaddr_un addr_client;
    memset(&addr_client, 0, sizeof(struct sockaddr_un)); //Clear structure
    addr_client.sun_family = AF_UNIX;
    strcpy(addr_client.sun_path, "/proc/self/root");
    strcat(addr_client.sun_path, socket_ready_path);

    socklen_t client_addr_len = strlen(addr_client.sun_path) + sizeof(
                                    addr_client.sun_family) + 1;
    if (connect(client_fd, (struct sockaddr *)&addr_client, client_addr_len) == -1) {
        close(listen_fd);
        close(client_fd);
        THROW_ERROR("failed to connect");
    }

    int accepted_fd = accept(listen_fd, (struct sockaddr *)&addr_client, &client_addr_len);
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

    struct pollfd polls[] = {
        { .fd = connected_sockets[1], .events = POLLIN },
    };

    // Test for blocking poll, poll will be only interrupted by sigchld
    // if socket does not support waking up a sleeping poller
    int ret = poll(polls, 1, -1);
    if (ret < 0) {
        THROW_ERROR("failed to poll");
    }

    char actual_str[32] = {0};
    ssize_t len = read(connected_sockets[1], actual_str, 32);
    if (len != sizeof(ECHO_MSG) || strncmp(actual_str, ECHO_MSG, strlen(ECHO_MSG)) != 0) {
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

// To emulate JVM bahaviour on UDS
int test_unix_socket_rename() {
    return test_connected_sockets_inter_process(create_connected_sockets_then_rename);
}

int test_poll() {
    int socks[2];
    if (socketpair(AF_UNIX, SOCK_STREAM, 0, socks) < 0) {
        THROW_ERROR("socketpair failed");
    }

    if (write(socks[0], "not today\n", 10) < 0) {
        THROW_ERROR("failed to write to socket");
    }

    struct pollfd polls[] = {
        { .fd = socks[0], .events = POLLOUT },
        { .fd = socks[1], .events = POLLIN },
    };

    int ret = poll(polls, 2, 5000);
    if (ret <= 0) { THROW_ERROR("poll error"); }
    if (((polls[0].revents & POLLOUT) && (polls[1].revents & POLLIN)) == 0) {
        printf("%d %d\n", polls[0].revents, polls[1].revents);
        THROW_ERROR("wrong return events");
    }
    return 0;
}

int test_getname() {
    char name[] = "unix_socket_path";
    int sock = socket(AF_UNIX, SOCK_STREAM, 0);
    if (sock == -1) {
        THROW_ERROR("failed to create a unix socket");
    }

    struct sockaddr_un addr = {0};
    memset(&addr, 0, sizeof(struct sockaddr_un)); //Clear structure
    addr.sun_family = AF_UNIX;
    strcpy(addr.sun_path, name);
    socklen_t addr_len = strlen(addr.sun_path) + sizeof(addr.sun_family) + 1;
    if (bind(sock, (struct sockaddr *)&addr, addr_len) == -1) {
        close(sock);
        THROW_ERROR("failed to bind");
    }

    struct sockaddr_un ret_addr = {0};
    socklen_t ret_addr_len = sizeof(ret_addr);

    if (getsockname(sock, (struct sockaddr *)&ret_addr, &ret_addr_len) < 0) {
        close(sock);
        THROW_ERROR("failed to getsockname");
    }

    if (ret_addr_len != addr_len || strcmp(ret_addr.sun_path, name) != 0) {
        close(sock);
        THROW_ERROR("got name mismatched");
    }

    close(sock);
    return 0;
}

int test_ioctl_fionread() {
    int ret = 0;
    int sockets[2];
    ret = socketpair(AF_UNIX, SOCK_STREAM, 0, sockets);
    if (ret < 0) {
        THROW_ERROR("failed to create a unix socket");
    }

    const char *child_prog = "/bin/hello_world";
    const char *child_argv[3] = { child_prog, ECHO_MSG, NULL };
    int child_pid;
    posix_spawn_file_actions_t file_actions;

    posix_spawn_file_actions_init(&file_actions);
    posix_spawn_file_actions_adddup2(&file_actions, sockets[0], STDOUT_FILENO);
    posix_spawn_file_actions_addclose(&file_actions, sockets[1]);

    if (posix_spawn(&child_pid, child_prog, &file_actions,
                    NULL, (char *const *)child_argv, NULL) < 0) {
        THROW_ERROR("failed to spawn a child process");
    }

    int status = 0;
    if (wait4(child_pid, &status, 0, NULL) < 0) {
        THROW_ERROR("failed to wait4 the child process");
    }

    // data should be ready
    int data_len_ready = 0;
    if (ioctl(sockets[1], FIONREAD, &data_len_ready) < 0) {
        THROW_ERROR("failed to ioctl with FIONREAD option");
    }

    // data_len_ready will include '\0'
    if (data_len_ready - 1 != strlen(ECHO_MSG)) {
        THROW_ERROR("ioctl FIONREAD value not match");
    }

    char actual_str[32] = {0};
    ssize_t len = read(sockets[1], actual_str, 32);
    if (len != sizeof(ECHO_MSG) || strncmp(actual_str, ECHO_MSG, strlen(ECHO_MSG)) != 0) {
        printf("data read is :%s\n", actual_str);
        THROW_ERROR("received string is not as expected");
    }

    return 0;
}

static test_case_t test_cases[] = {
    TEST_CASE(test_unix_socket_inter_process),
    TEST_CASE(test_socketpair_inter_process),
    TEST_CASE(test_multiple_socketpairs),
    TEST_CASE(test_poll),
    TEST_CASE(test_getname),
    TEST_CASE(test_ioctl_fionread),
    TEST_CASE(test_unix_socket_rename),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
