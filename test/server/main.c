#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <spawn.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include <arpa/inet.h>
#include <netinet/in.h>
#include <sys/types.h>
#include <sys/socket.h>
#include <sys/wait.h>

#include "test.h"

#define ECHO_MSG "msg for client/server test"
#define RESPONSE "ACK"
#define DEFAULT_MSG "Hello World!\n"

int connect_with_child(int port, int *child_pid) {
    int ret = 0;
    int listen_fd = socket(AF_INET, SOCK_STREAM, 0);
    if (listen_fd < 0) {
        THROW_ERROR("create socket error");
    }
    int reuse = 1;
    if (setsockopt(listen_fd, SOL_SOCKET, SO_REUSEADDR, &reuse, sizeof(reuse)) < 0) {
        THROW_ERROR("setsockopt port to reuse failed");
    }

    struct sockaddr_in servaddr;
    memset(&servaddr, 0, sizeof(servaddr));
    servaddr.sin_family = AF_INET;
    servaddr.sin_addr.s_addr = htonl(INADDR_ANY);
    servaddr.sin_port = htons(port);
    ret = bind(listen_fd, (struct sockaddr *) &servaddr, sizeof(servaddr));
    if (ret < 0) {
        close(listen_fd);
        THROW_ERROR("bind socket failed");
    }

    ret = listen(listen_fd, 10);
    if (ret < 0) {
        close(listen_fd);
        THROW_ERROR("listen socket error");
    }

    char port_string[8];
    sprintf(port_string, "%d", port);
    char *client_argv[] = {"client", "127.0.0.1", port_string, NULL};
    ret = posix_spawn(child_pid, "/bin/client", NULL, NULL, client_argv, NULL);
    if (ret < 0) {
        close(listen_fd);
        THROW_ERROR("spawn client process error");
    }

    int connected_fd = accept(listen_fd, (struct sockaddr *) NULL, NULL);
    if (connected_fd < 0) {
        close(listen_fd);
        THROW_ERROR("accept socket error");
    }

    close(listen_fd);
    return connected_fd;
}

int neogotiate_msg(int client_fd) {
    char buf[16];
    if (write(client_fd, ECHO_MSG, strlen(ECHO_MSG)) < 0) {
        THROW_ERROR("write failed");
    }

    if (read(client_fd, buf, sizeof(RESPONSE)) < 0) {
        THROW_ERROR("read failed");
    }

    if (strncmp(buf, RESPONSE, sizeof(RESPONSE)) != 0) {
        THROW_ERROR("msg recv mismatch");
    }
    return 0;
}

int server_recv(int client_fd) {
    const int buf_size = 32;
    char buf[buf_size];

    if (recv(client_fd, buf, buf_size, 0) <= 0) {
        THROW_ERROR("msg recv failed");
    }

    if (strncmp(buf, ECHO_MSG, strlen(ECHO_MSG)) != 0) {
        THROW_ERROR("msg recv mismatch");
    }
    return 0;
}

int server_recvmsg(int client_fd) {
    int ret = 0;
    const int buf_size = 10;
    char buf[3][buf_size];
    struct msghdr msg;
    struct iovec iov[3];

    msg.msg_name  = NULL;
    msg.msg_namelen  = 0;
    iov[0].iov_base = buf[0];
    iov[0].iov_len = buf_size;
    iov[1].iov_base = buf[1];
    iov[1].iov_len = buf_size;
    iov[2].iov_base = buf[2];
    iov[2].iov_len = buf_size;
    msg.msg_iov = iov;
    msg.msg_iovlen = 3;
    msg.msg_control = 0;
    msg.msg_controllen = 0;
    msg.msg_flags = 0;

    ret = recvmsg(client_fd, &msg, 0);
    if (ret <= 0) {
        THROW_ERROR("recvmsg failed");
    } else {
        if (strncmp(buf[0], ECHO_MSG, buf_size) != 0 &&
                strstr(ECHO_MSG, buf[1]) != NULL &&
                strstr(ECHO_MSG, buf[2]) != NULL) {
            printf("recvmsg : %d, msg: %s,  %s, %s\n", ret, buf[0], buf[1], buf[2]);
            THROW_ERROR("msg recvmsg mismatch");
        }
    }
    msg.msg_iov = NULL;
    msg.msg_iovlen = 0;
    ret = recvmsg(client_fd, &msg, 0);
    if (ret != 0) {
        THROW_ERROR("recvmsg empty failed");
    }
    return ret;
}

int sigchld = 0;

void proc_exit() {
    sigchld = 1;
}

int server_connectionless_recvmsg() {
    int ret = 0;
    const int buf_size = 1000;
    char buf[buf_size];
    struct msghdr msg;
    struct iovec iov[1];

    struct sockaddr_in servaddr;
    struct sockaddr_in clientaddr;
    memset(&servaddr, 0, sizeof(servaddr));
    memset(&clientaddr, 0, sizeof(clientaddr));

    int sock = socket(AF_INET, SOCK_DGRAM, 0);
    if (sock < 0) {
        THROW_ERROR("create socket error");
    }

    servaddr.sin_family = AF_INET;
    servaddr.sin_addr.s_addr = htonl(INADDR_ANY);
    servaddr.sin_port = htons(9900);
    ret = bind(sock, (struct sockaddr *) &servaddr, sizeof(servaddr));
    if (ret < 0) {
        close(sock);
        THROW_ERROR("bind socket failed");
    }

    msg.msg_name  = &clientaddr;
    msg.msg_namelen  = sizeof(clientaddr);
    iov[0].iov_base = buf;
    iov[0].iov_len = buf_size;
    msg.msg_iov = iov;
    msg.msg_iovlen = 1;
    msg.msg_control = 0;
    msg.msg_controllen = 0;
    msg.msg_flags = 0;

    if (sigchld != 0) {
        return 0;
    }

    ret = recvmsg(sock, &msg, 0);
    if (ret <= 0) {
        THROW_ERROR("recvmsg failed");
    } else {
        if (strncmp(buf, DEFAULT_MSG, strlen(DEFAULT_MSG)) != 0) {
            printf("recvmsg : %d, msg: %s\n", ret, buf);
            THROW_ERROR("msg recvmsg mismatch");
        } else {
            inet_ntop(AF_INET, &clientaddr.sin_addr,
                      buf, sizeof(buf));
            if (strcmp(buf, "127.0.0.1") != 0) {
                printf("from port %d and address %s\n", ntohs(clientaddr.sin_port), buf);
                THROW_ERROR("client addr mismatch");
            }
        }
    }
    return ret;
}

int wait_for_child_exit(int child_pid) {
    int status = 0;
    if (wait4(child_pid, &status, 0, NULL) < 0) {
        THROW_ERROR("failed to wait4 the child process");
    }
    return 0;
}

int test_read_write() {
    int ret = 0;
    int child_pid = 0;
    int client_fd = connect_with_child(8800, &child_pid);
    if (client_fd < 0) {
        THROW_ERROR("connect failed");
    } else {
        ret = neogotiate_msg(client_fd);
    }

    //wait for the child to exit for next spawn
    int status = 0;
    if (wait4(child_pid, &status, 0, NULL) < 0) {
        THROW_ERROR("failed to wait4 the child process");
    }

    return ret;
}

int test_send_recv() {
    int ret = 0;
    int child_pid = 0;
    int client_fd = connect_with_child(8801, &child_pid);
    if (client_fd < 0) {
        THROW_ERROR("connect failed");
    }

    if (neogotiate_msg(client_fd) < 0) {
        THROW_ERROR("neogotiate failed");
    }

    ret = server_recv(client_fd);
    if (ret < 0) { return -1; }

    ret = wait_for_child_exit(child_pid);

    return ret;
}

int test_sendmsg_recvmsg() {
    int ret = 0;
    int child_pid = 0;
    int client_fd = connect_with_child(8802, &child_pid);
    if (client_fd < 0) {
        THROW_ERROR("connect failed");
    }

    if (neogotiate_msg(client_fd) < 0) {
        THROW_ERROR("neogotiate failed");
    }

    ret = server_recvmsg(client_fd);
    if (ret < 0) { return -1; }

    ret = wait_for_child_exit(child_pid);

    return ret;
}

int test_sendmsg_recvmsg_connectionless() {
    int ret = 0;
    int child_pid = 0;

    signal(SIGCHLD, proc_exit);

    char *client_argv[] = {"client", "NULL", "8803", NULL};
    ret = posix_spawn(&child_pid, "/bin/client", NULL, NULL, client_argv, NULL);
    if (ret < 0) {
        THROW_ERROR("spawn client process error");
    }

    ret = server_connectionless_recvmsg();
    if (ret < 0) { return -1; }

    ret = wait_for_child_exit(child_pid);

    return ret;
}

int test_fcntl_setfl_and_getfl() {
    int ret = 0;
    int child_pid = 0;
    int client_fd = -1;
    int original_flags, actual_flags;

    client_fd = connect_with_child(8804, &child_pid);
    if (client_fd < 0) {
        THROW_ERROR("connect failed");
    }
    original_flags = fcntl(client_fd, F_GETFL, 0);
    if (original_flags < 0) {
        THROW_ERROR("fcntl getfl failed");
    }

    ret = fcntl(client_fd, F_SETFL, original_flags | O_NONBLOCK);
    if (ret < 0) {
        THROW_ERROR("fcntl setfl failed");
    }

    actual_flags = fcntl(client_fd, F_GETFL, 0);
    if (actual_flags != (original_flags | O_NONBLOCK)) {
        THROW_ERROR("check the getfl value after setfl failed");
    }

    ret = wait_for_child_exit(child_pid);

    return ret;
}

int test_poll_events_unchanged() {
    int socks[2], ret;
    socks[0] = socket(AF_INET, SOCK_STREAM, 0);
    socks[1] = socket(AF_INET, SOCK_STREAM, 0);
    struct pollfd pollfds[] = {
        { .fd = socks[0], .events = POLLIN },
        { .fd = socks[1], .events = POLLIN },
    };

    ret = poll(pollfds, 2, 0);
    if (ret < 0) {
        THROW_ERROR("poll error");
    }

    if (pollfds[0].fd != socks[0] ||
            pollfds[0].events != POLLIN ||
            pollfds[1].fd != socks[1] ||
            pollfds[1].events != POLLIN) {
        THROW_ERROR("fd and events of pollfd should remain unchanged");
    }
    return 0;
}

int test_poll() {
    int child_pid = 0;
    int client_fd = connect_with_child(8805, &child_pid);
    if (client_fd < 0) {
        THROW_ERROR("connect failed");
    }

    struct pollfd polls[] = {
        { .fd = client_fd, .events = POLLIN }
    };
    int ret = poll(polls, 1, -1);
    if (ret <= 0) {
        THROW_ERROR("poll error");
    }

    if (polls[0].revents & POLLIN) {
        ssize_t count;
        char buf[512];
        if ((count = read(client_fd, buf, sizeof buf)) != 0) {
            if (strcmp(buf, DEFAULT_MSG) != 0) {
                printf(buf);
                THROW_ERROR("msg mismatched");
            }
        } else {
            THROW_ERROR("read error");
        }
    } else {
        THROW_ERROR("unexpected return events");
    }

    int status = 0;
    if (wait4(child_pid, &status, 0, NULL) < 0) {
        THROW_ERROR("failed to wait4 the child process");
    }
    close(client_fd);
    return 0;
}

static test_case_t test_cases[] = {
    TEST_CASE(test_read_write),
    TEST_CASE(test_send_recv),
    TEST_CASE(test_sendmsg_recvmsg),
    TEST_CASE(test_sendmsg_recvmsg_connectionless),
    TEST_CASE(test_fcntl_setfl_and_getfl),
    TEST_CASE(test_poll),
    TEST_CASE(test_poll_events_unchanged),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
