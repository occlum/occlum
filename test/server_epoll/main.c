#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <netdb.h>
#include <stdio.h>
#include <stdlib.h>
#include <spawn.h>
#include <string.h>
#include <unistd.h>
#include <sys/epoll.h>
#include <sys/socket.h>
#include <sys/types.h>
#include <sys/wait.h>

#include "test.h"

#define MAXEVENTS 64
#define MAXRETRY_TIMES 3
#define DEFAULT_PROC_NUM 3
#define DEFAULT_MSG "Hello World!\n"
// The recv buf length should be longer than that of DEFAULT_MSG
#define RECV_BUF_LENGTH 32

static int create_and_bind() {
    int listenfd = socket(AF_INET, SOCK_STREAM | SOCK_NONBLOCK, 0);
    if (listenfd < 0) {
        printf("create socket error: %s(errno: %d)\n", strerror(errno), errno);
        return -1;
    }

    struct sockaddr_in servaddr = {0};
    servaddr.sin_family = AF_INET;
    servaddr.sin_addr.s_addr = htonl(INADDR_ANY);
    servaddr.sin_port = htons(6667);

    int reuse = 1;
    if (setsockopt(listenfd, SOL_SOCKET, SO_REUSEADDR, &reuse, sizeof(reuse)) < 0) {
        THROW_ERROR("setsockopt port to reuse failed");
    }

    int ret = bind(listenfd, (struct sockaddr *) &servaddr, sizeof(servaddr));
    if (ret < 0) {
        printf("bind socket error: %s(errno: %d)\n", strerror(errno), errno);
        return -1;
    }
    return listenfd;
}

int test_ip_socket() {
    int ret = 0;
    int server_fd = create_and_bind();

    ret = listen(server_fd, DEFAULT_PROC_NUM);
    if (ret == -1) {
        THROW_ERROR("failed to listen");
    }

    int epfd = epoll_create1(0);
    if (epfd == -1) {
        close(server_fd);
        THROW_ERROR("epoll_create failed");
    }

    struct epoll_event listened_event;
    listened_event.data.fd = server_fd;
    listened_event.events = EPOLLIN | EPOLLET;
    ret = epoll_ctl(epfd, EPOLL_CTL_ADD, server_fd, &listened_event);
    if (ret == -1) {
        close_files(2, server_fd, epfd);
        THROW_ERROR("epoll_ctl failed");
    }

    int client_pid;
    int proc_num = DEFAULT_PROC_NUM;
    char *client_argv[] = {"client", "127.0.0.1", "6667", NULL};
    for (int i = 0; i < DEFAULT_PROC_NUM; ++i) {
        int ret = posix_spawn(&client_pid, "/bin/client", NULL, NULL, client_argv, NULL);
        if (ret < 0) {
            if (i == 0) {
                close_files(2, server_fd, epfd);
                THROW_ERROR("no client is successfully spawned");
            } else {
                printf("%d client(s) spawned\n", i);
                proc_num = i;
                break;
            }
        }
    }

    int count = 0;
    while (count < proc_num) {
        struct epoll_event events[MAXEVENTS] = {0};
        int retry_times = 0;
        int nfds = -1;
        while (1) {
            nfds = epoll_pwait(epfd, events, MAXEVENTS, -1, NULL);

            if (nfds >= 0) {
                break;
            } else if ( retry_times == MAXRETRY_TIMES ) {
                close_files(2, server_fd, epfd);
                THROW_ERROR("epoll_wait failed");
            }

            retry_times++;
        }

        for (int i = 0; i < nfds; i++) {
            if (server_fd == events[i].data.fd) {
                // There is incoming connection to server_fd.
                // Loop to accept all the connections.
                while (1) {
                    struct sockaddr in_addr = {0};
                    socklen_t in_len;
                    int in_fd;
                    in_len = sizeof(in_addr);
                    in_fd = accept4(server_fd, &in_addr, &in_len, SOCK_NONBLOCK);
                    if (in_fd == -1) {
                        if (errno == EAGAIN || errno == EWOULDBLOCK) {
                            // No pending connections are present.
                            break;
                        } else {
                            close_files(2, server_fd, epfd);
                            THROW_ERROR("unexpected accept error");
                        }
                    }

                    struct epoll_event client_event;
                    client_event.data.fd = in_fd;
                    client_event.events = EPOLLIN | EPOLLET;
                    ret = epoll_ctl(epfd, EPOLL_CTL_ADD, in_fd, &client_event);
                    if (ret == -1) {
                        close_files(2, server_fd, epfd);
                        THROW_ERROR("epoll_ctl failed");
                    }
                }
            } else if (events[i].events & EPOLLIN) {
                // Channel is ready to read.
                char buf[RECV_BUF_LENGTH];
                if ((read(events[i].data.fd, buf, sizeof buf)) != 0) {
                    if (strncmp(buf, DEFAULT_MSG, strlen(DEFAULT_MSG)) != 0) {
                        for (int i = 0; i < RECV_BUF_LENGTH; i++) {
                            printf("%c, ", buf[i]);
                        }
                        close_files(2, server_fd, epfd);
                        THROW_ERROR("msg mismatched");
                    }
                } else {
                    close_files(2, server_fd, epfd);
                    THROW_ERROR("read error");
                }

                close(events[i].data.fd);
                // Finish communication with one process.
                count++;
            } else {
                close_files(2, server_fd, epfd);
                THROW_ERROR("should never reach here");
            }
        }
    }

    // Wait for all the children to exit
    for (int i = 0; i < proc_num; i++) {
        if (wait(NULL) < 0) {
            close_files(2, server_fd, epfd);
            THROW_ERROR("failed to wait");
        }
    }

    close_files(2, server_fd, epfd);
    return 0;
}

static test_case_t test_cases[] = {
    TEST_CASE(test_ip_socket),
};

int main(int argc, const char *argv[]) {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
