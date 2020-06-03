#include <stdio.h>
#include <stdlib.h>
#include <netinet/in.h>
#include <sys/socket.h>
#include <arpa/inet.h>
#include <string.h>
#include <unistd.h>
#include <errno.h>
#include <sys/un.h>
#include <sys/select.h>
#include <sys/time.h>
#include "fifo_def.h"
#include "proc_msg.h"

#define BACKLOG 5
#define CONCURRENT_MAX 32
#define SERVER_PORT 8888
#define BUFFER_SIZE 1024

int m_server_sock_fd;
int m_shutdown;

int server_init() {
    struct sockaddr_in srv_addr;

    memset(&srv_addr, 0, sizeof(srv_addr));
    srv_addr.sin_family = AF_INET;
    srv_addr.sin_addr.s_addr = INADDR_ANY;
    srv_addr.sin_port = htons(SERVER_PORT);

    m_server_sock_fd = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);;
    if (m_server_sock_fd == -1) {
        printf("socket initiazation error: %d\n", errno);
        return -1;
    }

    int bind_result = bind(m_server_sock_fd, (struct sockaddr *)&srv_addr, sizeof(srv_addr));
    if (bind_result == -1) {
        printf("bind error: %d\n", errno);
        close(m_server_sock_fd);
        return -1;
    }

    if (listen(m_server_sock_fd, BACKLOG) == -1) {
        printf("listen error: %d\n", errno);
        close(m_server_sock_fd);
        return -1;
    }

    m_shutdown = 0;
    return 0;
}

int work() {
    int client_fds[CONCURRENT_MAX] = {0};
    fd_set server_fd_set;
    int max_fd = -1;
    struct timeval tv;
    char input_msg[BUFFER_SIZE];
    char recv_msg[BUFFER_SIZE];
    int ret;

    while (!m_shutdown) {
        // set 10s timeout for select()
        tv.tv_sec = 10;
        tv.tv_usec = 0;
        FD_ZERO(&server_fd_set);
        // listening on server socket
        FD_SET(m_server_sock_fd, &server_fd_set);
        if (max_fd < m_server_sock_fd) {
            max_fd = m_server_sock_fd;
        }

        // listening on all client connections
        for (int i = 0; i < CONCURRENT_MAX; i++) {
            if (client_fds[i] != 0) {
                FD_SET(client_fds[i], &server_fd_set);
                if (max_fd < client_fds[i]) {
                    max_fd = client_fds[i];
                }
            }
        }

        ret = select(max_fd + 1, &server_fd_set, NULL, NULL, &tv);
        if (ret < 0) {
            printf("Warning: server would shutdown\n");
            continue;
        } else if (ret == 0) {
            // timeout
            continue;
        }

        if (FD_ISSET(m_server_sock_fd, &server_fd_set)) {
            // if there is new connection request
            struct sockaddr_in clt_addr;
            socklen_t len = sizeof(clt_addr);

            // accept this connection request
            int client_sock_fd = accept(m_server_sock_fd, (struct sockaddr *)&clt_addr, &len);

            if (client_sock_fd > 0) {
                // add new connection to connection pool if it's not full
                int index = -1;
                for (int i = 0; i < CONCURRENT_MAX; i++) {
                    if (client_fds[i] == 0) {
                        index = i;
                        client_fds[i] = client_sock_fd;
                        break;
                    }
                }

                if (index < 0) {
                    printf("server reach maximum connection!\n");
                    bzero(input_msg, BUFFER_SIZE);
                    strcpy(input_msg, "server reach maximum connection\n");
                    send(client_sock_fd, input_msg, BUFFER_SIZE, 0);
                }
            } else if (client_sock_fd < 0) {
                printf("server: accept() return failure, %s, would exit.\n", strerror(errno));
                close(m_server_sock_fd);
                break;
            }
        }

        for (int i = 0; i < CONCURRENT_MAX; i++) {
            if ((client_fds[i] != 0)
                    && (FD_ISSET(client_fds[i], &server_fd_set))) {
                // there is request messages from client connectsions
                FIFO_MSG *msg;
                bzero(recv_msg, BUFFER_SIZE);
                long byte_num = recv(client_fds[i], recv_msg, BUFFER_SIZE, 0);
                if (byte_num > 0) {
                    if (byte_num > BUFFER_SIZE) {
                        byte_num = BUFFER_SIZE;
                    }

                    recv_msg[byte_num] = '\0';
                    msg = (FIFO_MSG *)malloc(byte_num);
                    if (!msg) {
                        printf("memory allocation failure\n");
                        continue;
                    }
                    memset(msg, 0, byte_num);
                    memcpy(msg, recv_msg, byte_num);
                    msg->header.sockfd = client_fds[i];
                    proc(msg);
                } else if (byte_num < 0) {
                    printf("failed to receive message.\n");
                } else {
                    // client connect is closed
                    FD_CLR(client_fds[i], &server_fd_set);
                    close(client_fds[i]);
                    client_fds[i] = 0;
                }
            }
        }
    }
}

int main() {
    int rc;

    rc = server_init();
    if (rc != 0) {
        printf("server init failure\n");
        return rc;
    }

    return work();
}
