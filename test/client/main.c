#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <sys/types.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <spawn.h>
#include <unistd.h>

#include "test.h"

#define RESPONSE "ACK"
#define DEFAULT_MSG "Hello World!\n"

int connect_with_server(const char *addr_string, const char *port_string) {
    //"NULL" addr means connectionless, no need to connect to server
    if (strcmp(addr_string, "NULL") == 0) {
        return 0;
    }

    int ret = 0;
    int sockfd = socket(AF_INET, SOCK_STREAM, 0);
    if (sockfd < 0) {
        THROW_ERROR("create socket error");
    }

    struct sockaddr_in servaddr;
    memset(&servaddr, 0, sizeof(servaddr));
    servaddr.sin_family = AF_INET;
    servaddr.sin_port = htons((uint16_t)strtol(port_string, NULL, 10));
    ret = inet_pton(AF_INET, addr_string, &servaddr.sin_addr);
    if (ret <= 0) {
        close(sockfd);
        THROW_ERROR("inet_pton error");
    }

    ret = connect(sockfd, (struct sockaddr *) &servaddr, sizeof(servaddr));
    if (ret < 0) {
        close(sockfd);
        THROW_ERROR("connect error");
    }

    return sockfd;
}

int neogotiate_msg(int server_fd, char *buf, int buf_size) {
    if (read(server_fd, buf, buf_size) < 0) {
        THROW_ERROR("read failed");
    }

    if (write(server_fd, RESPONSE, sizeof(RESPONSE)) < 0) {
        THROW_ERROR("write failed");
    }
    return 0;
}

int client_send(int server_fd, char *buf) {
    if (send(server_fd, buf, strlen(buf), 0) < 0) {
        THROW_ERROR("send msg error");
    }
    return 0;
}

int client_sendmsg(int server_fd, char *buf) {
    int ret = 0;
    struct msghdr msg;
    struct iovec iov[1];
    msg.msg_name = NULL;
    msg.msg_namelen = 0;
    iov[0].iov_base = buf;
    iov[0].iov_len = strlen(buf);
    msg.msg_iov = iov;
    msg.msg_iovlen = 1;
    msg.msg_control = 0;
    msg.msg_controllen = 0;
    msg.msg_flags = 0;

    ret = sendmsg(server_fd, &msg, 0);
    if (ret <= 0) {
        THROW_ERROR("sendmsg failed");
    }

    msg.msg_iov = NULL;
    msg.msg_iovlen = 0;

    ret = sendmsg(server_fd, &msg, 0);
    if (ret != 0) {
        THROW_ERROR("empty sendmsg failed");
    }
    return ret;
}

#ifdef __GLIBC__

int client_sendmmsg(int server_fd, char *buf) {
    int ret = 0;
    struct mmsghdr msg_v[2] = {};
    struct iovec iov[1];
    struct msghdr *msg_ptr = &msg_v[0].msg_hdr;

    // Set msg0
    msg_ptr->msg_name = NULL;
    msg_ptr->msg_namelen = 0;
    iov[0].iov_base = buf;
    iov[0].iov_len = strlen(buf);
    msg_ptr->msg_iov = iov;
    msg_ptr->msg_iovlen = 1;
    msg_ptr->msg_control = 0;
    msg_ptr->msg_controllen = 0;
    msg_ptr->msg_flags = 0;

    // Set msg1
    msg_v[1] = msg_v[0];
    msg_ptr = &msg_v[1].msg_hdr;
    msg_ptr->msg_iov = NULL;
    msg_ptr->msg_iovlen = 0;

    ret = sendmmsg(server_fd, msg_v,  2, 0);
    if (ret != 2 || msg_v[0].msg_len <= 0 || msg_v[1].msg_len != 0) {
        THROW_ERROR("sendmsg failed");
    }
    return 0;
}
#endif

int client_connectionless_sendmsg(char *buf) {
    int ret = 0;
    struct msghdr msg;
    struct iovec iov[1];
    struct sockaddr_in servaddr;
    memset(&servaddr, 0, sizeof(servaddr));

    servaddr.sin_family = AF_INET;
    servaddr.sin_port = htons(9900);
    servaddr.sin_addr.s_addr = htonl(INADDR_ANY);

    msg.msg_name = &servaddr;
    msg.msg_namelen = sizeof(servaddr);
    iov[0].iov_base = buf;
    iov[0].iov_len = strlen(buf);
    msg.msg_iov = iov;
    msg.msg_iovlen = 1;
    msg.msg_control = 0;
    msg.msg_controllen = 0;
    msg.msg_flags = 0;

    int server_fd = socket(AF_INET, SOCK_DGRAM, 0);
    if (server_fd < 0) {
        THROW_ERROR("create socket error");
    }

    ret = sendmsg(server_fd, &msg, 0);
    if (ret <= 0) {
        THROW_ERROR("sendmsg failed");
    }
    return ret;
}

int main(int argc, const char *argv[]) {
    if (argc != 3) {
        THROW_ERROR("usage: ./client <ipaddress> <port>\n");
    }

    int ret = 0;
    const int buf_size = 100;
    char buf[buf_size];
    int port = strtol(argv[2], NULL, 10);
    int server_fd = connect_with_server(argv[1], argv[2]);

    switch (port) {
        case 8800:
            neogotiate_msg(server_fd, buf, buf_size);
            break;
        case 8801:
            neogotiate_msg(server_fd, buf, buf_size);
            ret = client_send(server_fd, buf);
            break;
        case 8802:
            neogotiate_msg(server_fd, buf, buf_size);
            ret = client_sendmsg(server_fd, buf);
            break;
#ifdef __GLIBC__
        case 8803:
            neogotiate_msg(server_fd, buf, buf_size);
            ret = client_sendmmsg(server_fd, buf);
#endif
        case 8804:
            ret = client_connectionless_sendmsg(DEFAULT_MSG);
            break;
        default:
            ret = client_send(server_fd, DEFAULT_MSG);
    }

    close(server_fd);
    return ret;
}
