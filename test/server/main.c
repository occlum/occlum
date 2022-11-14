#define _GNU_SOURCE
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
#include <sys/epoll.h>
#include <time.h>
#include <pthread.h>

#include "test.h"

#define ECHO_MSG "msg for client/server test"
#define RESPONSE "ACK"
#define DEFAULT_MSG "Hello World!\n"
#define SYNC_MSG "sync"

#define CLIENT_FD 98
int pipe_fds[2];

int connect_with_child(int port, int *child_pid) {
    int ret = 0;

    if (pipe(pipe_fds) < 0) {
        THROW_ERROR("failed to create a pipe");
    }

    int pipe_rd_fd = pipe_fds[0];
    int pipe_wr_fd = pipe_fds[1];

    posix_spawn_file_actions_t file_actions;
    posix_spawn_file_actions_init(&file_actions);
    posix_spawn_file_actions_adddup2(&file_actions, pipe_rd_fd, CLIENT_FD);
    posix_spawn_file_actions_addclose(&file_actions, pipe_wr_fd);

    int listen_fd = socket(AF_INET, SOCK_STREAM, 0);
    if (listen_fd < 0) {
        THROW_ERROR("create socket error");
    }
    int reuse = 1;
    if (setsockopt(listen_fd, SOL_SOCKET, SO_REUSEADDR, &reuse, sizeof(reuse)) < 0) {
        THROW_ERROR("setsockopt port to reuse failed");
    }
    if (setsockopt(listen_fd, SOL_SOCKET, SO_REUSEPORT, &reuse, sizeof(reuse)) < 0) {
        THROW_ERROR("setsockopt port to reuse failed");
    }

    struct sockaddr_in servaddr;
    memset(&servaddr, 0, sizeof(servaddr));
    servaddr.sin_family = AF_INET;
    servaddr.sin_addr.s_addr = htonl(INADDR_ANY);
    servaddr.sin_port = htons(port);
    ret = bind(listen_fd, (struct sockaddr *)&servaddr, sizeof(servaddr));
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
    ret = posix_spawn(child_pid, "/bin/client", &file_actions, NULL, client_argv, NULL);
    if (ret < 0) {
        close(listen_fd);
        THROW_ERROR("spawn client process error");
    }

    close(pipe_rd_fd);

    int connected_fd = accept(listen_fd, (struct sockaddr *)NULL, NULL);
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

    msg.msg_name = NULL;
    msg.msg_namelen = 0;
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

int server_recvmsg_big_buf(int client_fd) {
    int ret = 0;
    const int buf_size = 128 * 1024;
    char *buffer[2];
    struct msghdr msg;
    struct iovec iov[2];
    int total_len = 0;
    char *check_buf = (char *)malloc(buf_size);

    // Set the two buffers to random value
    int fd = open("/dev/urandom", O_RDONLY);
    for (int i = 0; i < 2; i++) {
        char *buf = (char *)malloc(buf_size);
        if (read(fd, buf, buf_size) < 0) {
            THROW_ERROR("read /dev/urandom failure");
        }
        buffer[i] = buf;
    }
    // Check buffer is set to the same value as client send buffer
    memset(check_buf, 'a', buf_size);

    msg.msg_name = NULL;
    msg.msg_namelen = 0;
    iov[0].iov_base = buffer[0];
    iov[0].iov_len = buf_size;
    iov[1].iov_base = buffer[1];
    iov[1].iov_len = buf_size;
    msg.msg_iov = iov;
    msg.msg_iovlen = 2;
    msg.msg_control = 0;
    msg.msg_controllen = 0;
    msg.msg_flags = 0;

    while (total_len < buf_size * 2) {
        ret = recvmsg(client_fd, &msg, 0);
        if (ret < 0) {
            THROW_ERROR("recvmsg failed");
        }
        total_len += ret;
        // Update the iov and msg
        if (total_len < buf_size) {
            iov[0].iov_base = buffer[0] + total_len;
            iov[0].iov_len = buf_size - total_len;
        } else {
            int index = total_len - buf_size;
            iov[1].iov_base = buffer[1] + index;
            iov[1].iov_len = buf_size - index;
            msg.msg_iov = iov + 1;
            msg.msg_iovlen = 1;
        }
    }

    if (strncmp(buffer[0], check_buf, buf_size) != 0 ||
            strncmp(buffer[1], check_buf, buf_size) != 0 ) {
        printf("recvmsg : %d, msg: %s,  %s\n", total_len, buffer[0], buffer[1]);
        THROW_ERROR("msg recvmsg mismatch");
    }

    return ret;
}

int sigchld = 0;

void proc_exit() {
    sigchld = 1;
}

int server_connectionless_recvmsg(int sock) {
    int ret = 0;
    const int buf_size = 1000;
    char buf[buf_size];
    struct msghdr msg;
    struct iovec iov[1];
    struct sockaddr_in clientaddr;
    memset(&clientaddr, 0, sizeof(clientaddr));

    msg.msg_name = &clientaddr;
    msg.msg_namelen = sizeof(clientaddr);
    iov[0].iov_base = buf;
    iov[0].iov_len = buf_size;
    msg.msg_iov = iov;
    msg.msg_iovlen = 1;
    msg.msg_control = 0;
    msg.msg_controllen = 0;
    msg.msg_flags = 0;

    ret = recvmsg(sock, &msg, 0);
    if (ret < 0 ) {
        if (errno != EINTR) {
            THROW_ERROR("recvmsg failed");
        } else {
            return 0;
        }
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
    int pipe_wr_fd = pipe_fds[1];
    char finish_str[] = "finished";

    if (write(pipe_wr_fd, finish_str, sizeof(finish_str)) < 0) {
        THROW_ERROR("failed to write");
    }
    close(pipe_wr_fd);

    if (wait4(child_pid, &status, 0, NULL) < 0) {
        THROW_ERROR("failed to wait4 the child process");
    }

    return 0;
}

static void *thread_wait_func(void *_arg) {
    pid_t *client_pid = _arg;

    waitpid(*client_pid, NULL, 0);

    return NULL;
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
    wait_for_child_exit(child_pid);
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
    if (ret < 0) {
        return -1;
    }

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
    if (ret < 0) {
        return -1;
    }

    ret = wait_for_child_exit(child_pid);

    return ret;
}

int test_sendmmsg_recvmsg() {
    int ret = 0;
    int child_pid = 0;
    int client_fd = connect_with_child(8803, &child_pid);
    if (client_fd < 0) {
        THROW_ERROR("connect failed");
    }

    if (neogotiate_msg(client_fd) < 0) {
        THROW_ERROR("neogotiate failed");
    }

    ret = server_recvmsg(client_fd);
    if (ret < 0) {
        return -1;
    }

    ret = wait_for_child_exit(child_pid);

    return ret;
}

int test_sendmsg_recvmsg_big_buf() {
    int ret = 0;
    int child_pid = 0;
    int client_fd = connect_with_child(8809, &child_pid);
    if (client_fd < 0) {
        THROW_ERROR("connect failed");
    }

    if (neogotiate_msg(client_fd) < 0) {
        THROW_ERROR("neogotiate failed");
    }

    ret = server_recvmsg_big_buf(client_fd);
    if (ret < 0) {
        return -1;
    }

    ret = wait_for_child_exit(child_pid);

    return ret;
}

int test_sendmsg_recvmsg_connectionless() {
    int ret = 0;
    int child_pid = 0;
    struct sockaddr_in servaddr;
    memset(&servaddr, 0, sizeof(servaddr));

    signal(SIGCHLD, proc_exit);

    int sock = socket(AF_INET, SOCK_DGRAM, 0);
    if (sock < 0) {
        THROW_ERROR("create socket error");
    }
    int reuse = 1;
    if (setsockopt(sock, SOL_SOCKET, SO_REUSEADDR, &reuse, sizeof(reuse)) < 0) {
        THROW_ERROR("setsockopt port to reuse failed");
    }
    if (setsockopt(sock, SOL_SOCKET, SO_REUSEPORT, &reuse, sizeof(reuse)) < 0) {
        THROW_ERROR("setsockopt port to reuse failed");
    }

    servaddr.sin_family = AF_INET;
    servaddr.sin_addr.s_addr = htonl(INADDR_ANY);
    servaddr.sin_port = htons(9900);
    ret = bind(sock, (struct sockaddr *)&servaddr, sizeof(servaddr));
    if (ret < 0) {
        close(sock);
        THROW_ERROR("bind socket failed");
    }

    char *client_argv[] = {"client", "NULL", "8804", NULL, NULL};
    ret = posix_spawn(&child_pid, "/bin/client", NULL, NULL, client_argv, NULL);
    if (ret < 0) {
        THROW_ERROR("spawn client process error");
    }

    ret = server_connectionless_recvmsg(sock);

    /* If child client send happens before recvmsg, EINTR may
    be triggered which is not failed case */
    if (ret < 0 && errno != EINTR) {
        THROW_ERROR("server_connectionless_recvmsg failed");
    }

    int status = 0;
    if (wait4(child_pid, &status, 0, NULL) < 0) {
        THROW_ERROR("failed to wait4 the child process");
    }

    return ret;
}

int test_fcntl_setfl_and_getfl() {
    int ret = 0;
    int child_pid = 0;
    int client_fd = -1;
    int original_flags, actual_flags;

    client_fd = connect_with_child(8808, &child_pid);
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
        {.fd = socks[0], .events = POLLIN},
        {.fd = socks[1], .events = POLLIN},
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
        {.fd = client_fd, .events = POLLIN}
    };
    int ret = poll(polls, 1, -1);
    if (ret <= 0) {
        THROW_ERROR("poll error");
    }

    if (polls[0].revents & POLLIN) {
        ssize_t count;
        char buf[512];
        if ((count = read(client_fd, buf, sizeof buf)) != 0) {
            if (count != strlen(DEFAULT_MSG) || strncmp(buf, DEFAULT_MSG, strlen(DEFAULT_MSG)) != 0) {
                printf("%s", buf);
                THROW_ERROR("msg mismatched");
            }
        } else {
            THROW_ERROR("read error");
        }
    } else {
        THROW_ERROR("unexpected return events");
    }

    wait_for_child_exit(child_pid);

    close(client_fd);
    return 0;
}

int test_sockopt() {
    int fd = socket(AF_INET, SOCK_STREAM, 0);
    if (fd < 0) {
        THROW_ERROR("create socket error");
    }
    int reuse = 1;
    if (setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &reuse, sizeof(reuse)) < 0) {
        THROW_ERROR("setsockopt port to reuse failed");
    }

    int optval = 0;
    socklen_t optlen = sizeof(optval);
    if (getsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &optval, &optlen) < 0 ||
            optval != 1) {
        THROW_ERROR("getsockopt(SO_REUSEADDR) failed");
    }

    optval = 0;
    optlen = sizeof(optval);
    if (getsockopt(fd, SOL_SOCKET, SO_DOMAIN, &optval, &optlen) < 0 ||
            optval != AF_INET) {
        THROW_ERROR("getsockopt(SO_DOMAIN) failed");
    }

    close(fd);
    return 0;
}

int server_getpeername(int client_fd) {
    struct sockaddr_in peer;
    socklen_t peer_len = sizeof(peer);
    if (getpeername(client_fd, (struct sockaddr *)&peer, &peer_len) < 0) {
        THROW_ERROR("getpeername() failed");
    }
    printf("Peer address: %s\n", inet_ntoa(peer.sin_addr));
    printf("Peer port: %d\n", (int)ntohs(peer.sin_port));

    struct sockaddr_in peer2;
    socklen_t peer_len2 = sizeof(peer2);
    if (getsockopt(client_fd, SOL_SOCKET, SO_PEERNAME, (struct sockaddr *)&peer2,
                   &peer_len2) < 0) {
        THROW_ERROR("getsockopt(SO_PEERNAME) failed");
    }
    if (strcmp(inet_ntoa(peer.sin_addr), inet_ntoa(peer2.sin_addr)) != 0 ||
            peer.sin_port != peer2.sin_port ||
            peer_len != peer_len2) {
        THROW_ERROR("the result of getsockopt(SO_PEERNAME) and getpeername is different");
    }
    return 0;
}

int test_getname() {
    int child_pid = 0;
    int client_fd = connect_with_child(8806, &child_pid);
    if (client_fd < 0) {
        THROW_ERROR("connect failed");
    }

    struct sockaddr_in myaddr;
    socklen_t myaddr_len = sizeof(myaddr);
    if (getsockname(client_fd, (struct sockaddr *)&myaddr, &myaddr_len) < 0) {
        THROW_ERROR("getsockname() failed");
    }
    printf("[socket with bind] address: %s\n", inet_ntoa(myaddr.sin_addr));
    printf("[socket with bind] port: %d\n", (int)ntohs(myaddr.sin_port));

    if (server_getpeername(client_fd) < 0) {
        THROW_ERROR("server_getpeername failed");
    }

    wait_for_child_exit(child_pid);

    close(client_fd);
    return 0;
}

int test_getname_without_bind() {
    int fd = socket(AF_INET, SOCK_STREAM, 0);

    struct sockaddr_in myaddr;
    socklen_t myaddr_len = sizeof(myaddr);
    if (getsockname(fd, (struct sockaddr *)&myaddr, &myaddr_len) < 0) {
        THROW_ERROR("getsockname() failed");
    }
    printf("[socket without bind] address: %s\n", inet_ntoa(myaddr.sin_addr));
    printf("[socket without bind] port: %d\n", (int)ntohs(myaddr.sin_port));

    struct sockaddr_in peer;
    socklen_t peer_len = sizeof(peer);
    if (getpeername(fd, (struct sockaddr *)&peer, &peer_len) == 0) {
        THROW_ERROR("getpeername() should failed");
    }

    struct sockaddr_in peer2;
    socklen_t peer_len2 = sizeof(peer2);
    if (getsockopt(fd, SOL_SOCKET, SO_PEERNAME, (struct sockaddr *)&peer2, &peer_len2) == 0) {
        THROW_ERROR("getsockopt(SO_PEERNAME) should failed");
    }

    close(fd);
    return 0;
}

int test_shutdown() {
    int fd = socket(AF_INET, SOCK_STREAM, 0);
    if (shutdown(fd, SHUT_RDWR) == 0) {
        THROW_ERROR("shutdown should return error");
    }

    int child_pid = 0;
    int client_fd = connect_with_child(8807, &child_pid);

    if (shutdown(client_fd, SHUT_RDWR) < 0) {
        THROW_ERROR("failed to shutdown");
    }

    wait_for_child_exit(child_pid);

    close(client_fd);
    return 0;
}

// the MSG_WAITALL test
char *msg[] = {"This is message 1", "...and this is message 2", "and this is the last message."};

void *connection_routine(void *arg) {
    int socketFd = *((int *)arg);
    sleep(1);

    int msg_count = 0;
    for (;;) {
        uint16_t len;
        int byteCount = recv(socketFd, &len, sizeof(len), MSG_WAITALL);
        if (byteCount < 1) {
            break;
        }
        char buff[1024] = {
            0,
        };
        byteCount = recv(socketFd, buff, ntohs(len), MSG_WAITALL);
        if (byteCount < 1) {
            break;
        }

        if (strncmp(msg[msg_count], buff, strlen(msg[msg_count])) != 0) {
            printf("message is wrong!\n");
            return NULL;
        }

        msg_count++;
        if (msg_count == 3) {
            break;
        }
    }

    close(socketFd);
    return NULL;
}

/*
 * NOT A GOOD IDEA for general use.
 * But to validate the receiver's use of MSG_WAITALL,
 * send one byte at a time.
 */
void writeMsg(int socketFd, size_t ln, char msg[]) {
    uint16_t netLn = htons(ln);
    send(socketFd, &netLn, 1, 0);
    char *arr = (char *)&netLn;
    send(socketFd, &arr[1], 1, 0);
    for (unsigned int i = 0; i < ln; i++) {
        send(socketFd, &msg[i], 1, 0);
    }
}

void *client_routine(void *arg) {
    struct timespec ts = {0, 1};
    nanosleep(&ts, NULL);

    uint16_t port = *((uint16_t *)arg);
    int sockFd = socket(AF_INET, SOCK_STREAM, 0);
    if (sockFd < 0) {
        printf("connectToTcp: error in socket(), %s\n", strerror(errno));
        return (void *) -1;
    }

    struct sockaddr_in sockAdr;

    memset(&sockAdr, 0, sizeof(struct sockaddr_in));
    sockAdr.sin_port = htons(port);
    sockAdr.sin_family = AF_INET;
    sockAdr.sin_addr.s_addr = inet_addr("127.0.0.1");

    int connRtn = connect(sockFd, (struct sockaddr *)&sockAdr,
                          sizeof(sockAdr));

    if (connRtn != 0) {
        printf("clientRoutine: error in connec");
        return NULL;
    }

    switch (port) {
        case 54321: { // for test_MSG_WAITALL
            writeMsg(sockFd, strlen(msg[0]), msg[0]);
            writeMsg(sockFd, strlen(msg[1]), msg[1]);
            writeMsg(sockFd, strlen(msg[2]), msg[2]);
            break;
        };
        case 54322: { // for test_epoll_wait
            sleep(2);
            if (write(sockFd, msg[0], strlen(msg[0])) < 0) {
                printf("write error: %s\n", strerror(errno));
                return (void *) -1;
            }
            break;
        };
    }

    shutdown(sockFd, SHUT_RDWR);

    close(sockFd);

    return NULL;
}

void *server_routine(void *arg) {
    uint16_t port = *((uint16_t *)arg);

    int sockfd = socket(AF_INET, SOCK_STREAM, 0);
    if (sockfd < 0) {
        printf("server_routine, error creating socket");
        return NULL;
    }

    struct sockaddr_in serv_addr = {};

    serv_addr.sin_family = AF_INET;
    serv_addr.sin_addr.s_addr = INADDR_ANY;
    serv_addr.sin_port = htons(port);

    if (bind(sockfd, (struct sockaddr *)&serv_addr, sizeof(serv_addr)) < 0) {
        printf("server_routine, error binding socket");
        return NULL;
    }

    if (listen(sockfd, 5) != 0) {
        printf("server_routine, error in listen");
        return NULL;
    }

    pthread_t client_tid;
    if (pthread_create(&client_tid, NULL, client_routine, (void *)&port)) {
        printf("Failure creating client thread");
        return NULL;
    }

    for (;;) {
        struct sockaddr_in saddr;
        unsigned int saddr_ln = sizeof(saddr);

        int newsock = accept(sockfd, (struct sockaddr *)&saddr, &saddr_ln);
        if (newsock == -1) {
            printf("server_routine, error in accept");
            return NULL;
        }
        pthread_t child_tid;
        if (pthread_create(&child_tid, NULL, connection_routine, (void *)&newsock)) {
            printf("Failure creating connection thread");
            return NULL;
        }

        pthread_join(child_tid, NULL);
        break;
    }

    pthread_join(client_tid, NULL);
    return NULL;
}

int test_MSG_WAITALL() {
    const uint16_t DEFAULT_PORT = 54321;
    uint16_t port = DEFAULT_PORT;

    pthread_t server_tid;
    if (pthread_create(&server_tid, NULL, server_routine, (void *)&port)) {
        THROW_ERROR("Failure creating server thread");
    }

    pthread_join(server_tid, NULL);
    return 0;
}

int test_epoll_wait() {
    struct sockaddr_in serv_addr = {};
    int port = 54322;
    int ret;
    struct epoll_event event;
    uint32_t interest_events = EPOLLIN;
    pthread_t client_tid;
    struct sockaddr_in saddr;
    unsigned int saddr_ln = sizeof(saddr);
    struct epoll_event polled_events;
    char read_buf[10];

    int sockfd = socket(AF_INET, SOCK_STREAM, 0);
    if (sockfd < 0) {
        THROW_ERROR("server_routine, error creating socket");
    }

    serv_addr.sin_family = AF_INET;
    serv_addr.sin_addr.s_addr = INADDR_ANY;
    serv_addr.sin_port = htons(port);

    if (bind(sockfd, (struct sockaddr *)&serv_addr, sizeof(serv_addr)) < 0) {
        THROW_ERROR("server_routine, error binding socket");
    }

    if (listen(sockfd, 5) != 0) {
        THROW_ERROR("server_routine, error in listen");
    }

    int ep_fd = epoll_create1(0);
    if (ep_fd < 0) {
        THROW_ERROR("failed to create an epoll");
    }

    if (pthread_create(&client_tid, NULL, client_routine, (void *)&port)) {
        THROW_ERROR("Failure creating client thread");
    }

    int newsock = accept(sockfd, (struct sockaddr *)&saddr, &saddr_ln);
    if (newsock == -1) {
        THROW_ERROR("server_routine, error in accept");
    }

    event.events = interest_events;
    event.data.u32 = newsock;
    ret = epoll_ctl(ep_fd, EPOLL_CTL_ADD, newsock, &event);
    if (ret < 0) {
        THROW_ERROR("failed to do epoll ctl");
    }

    // write to socket before epoll wait
    ret = write(newsock, msg[1], strlen(msg[1]));
    if (ret < 0) {
        THROW_ERROR("failed to write");
    }

    // wait infinitely
    ret = epoll_wait(ep_fd, &polled_events, 1, -1);
    if (ret != 1) {
        THROW_ERROR("failed to do epoll wait");
    }

    if (polled_events.events != interest_events) {
        THROW_ERROR("bad epoll event");
    }

    ret = read(newsock, read_buf, sizeof(read_buf));
    if (ret < 0) {
        THROW_ERROR("failed to read");
    }

    pthread_join(client_tid, NULL);
    return 0;
}

// This is a testcase mocking pyspark exit procedure. Client process is receiving and blocking.
// One of server process' child thread waits for the client to exit and the main thread calls exit_group.
static int test_exit_group() {
    int port = 8888;
    int pipes[2];
    int ret = 0;
    int listen_fd = socket(AF_INET, SOCK_STREAM, 0);
    if (listen_fd < 0) {
        THROW_ERROR("create socket error");
    }

    ret = pipe2(pipes, 0);
    if (ret < 0) {
        THROW_ERROR("error happens");
    }

    printf("pipe fd = %d, %d\n", pipes[0], pipes[1]);

    int child_pid = vfork();
    if (child_pid == 0) {
        ret = close(pipes[1]);
        if (ret < 0) {
            THROW_ERROR("error happens");
        }
        ret = dup2(pipes[0], 0);
        if (ret < 0) {
            THROW_ERROR("error happens");
        }

        ret = close(pipes[0]);
        if (ret < 0) {
            THROW_ERROR("error happens");
        }

        char port_string[8];
        sprintf(port_string, "%d", port);
        char *client_argv[] = {"client", "127.0.0.1", port_string, NULL};
        printf("exec child\n");
        execve("/bin/client", client_argv, NULL);
    }

    printf("return to parent\n");
    close(pipes[0]);

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

    ret = listen(listen_fd, 5);
    if (ret < 0) {
        close(listen_fd);
        THROW_ERROR("listen socket error");
    }

    int connected_fd = accept(listen_fd, (struct sockaddr *) NULL, NULL); // 4
    if (connected_fd < 0) {
        close(listen_fd);
        THROW_ERROR("accept socket error");
    }

    if (neogotiate_msg(connected_fd) < 0) {
        THROW_ERROR("neogotiate failed");
    }

    pthread_t tid;
    ret = pthread_create(&tid, NULL, thread_wait_func, &child_pid);
    if (ret != 0) {
        THROW_ERROR("create child error");
    }

    // Wait a while here for client to call recvfrom and blocking
    sleep(2);
    return 0;
}

static test_case_t test_cases[] = {
    TEST_CASE(test_MSG_WAITALL),
    TEST_CASE(test_read_write),
    TEST_CASE(test_send_recv),
    TEST_CASE(test_sendmsg_recvmsg),
#ifdef __GLIBC__
    TEST_CASE(test_sendmmsg_recvmsg),
#endif
    TEST_CASE(test_sendmsg_recvmsg_big_buf),
    TEST_CASE(test_sendmsg_recvmsg_connectionless),
    TEST_CASE(test_fcntl_setfl_and_getfl),
    TEST_CASE(test_poll),
    TEST_CASE(test_poll_events_unchanged),
    TEST_CASE(test_sockopt),
    TEST_CASE(test_getname),
    TEST_CASE(test_getname_without_bind),
    TEST_CASE(test_shutdown),
    TEST_CASE(test_epoll_wait),
    TEST_CASE(test_exit_group),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
