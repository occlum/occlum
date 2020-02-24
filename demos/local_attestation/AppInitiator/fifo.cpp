/*
 * Copyright (C) 2011-2019 Intel Corporation. All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 *
 *   * Redistributions of source code must retain the above copyright
 *     notice, this list of conditions and the following disclaimer.
 *   * Redistributions in binary form must reproduce the above copyright
 *     notice, this list of conditions and the following disclaimer in
 *     the documentation and/or other materials provided with the
 *     distribution.
 *   * Neither the name of Intel Corporation nor the names of its
 *     contributors may be used to endorse or promote products derived
 *     from this software without specific prior written permission.
 *
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
 * "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
 * LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
 * A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
 * OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
 * SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
 * LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
 * DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
 * THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
 * (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
 * OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 *
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <errno.h>
#include <sys/types.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <errno.h>

#include "fifo_def.h"

#define SERVER_ADDR "127.0.0.1"
#define SERVER_PORT 8888

#define BUFFER_SIZE 1024

/* Function Description: this is for client to send request message and receive response message
 * Parameter Description:
 * [input] fiforequest: this is pointer to request message
 * [input] fiforequest_size: this is request message size
 * [output] fiforesponse: this is pointer fo response message, the buffer is allocated inside this function
 * [output] fiforesponse_size: this is response message size
 * */
int client_send_receive(FIFO_MSG *fiforequest, size_t fiforequest_size, FIFO_MSG **fiforesponse, size_t *fiforesponse_size) {
    int ret = 0;
    long byte_num;
    char recv_msg[BUFFER_SIZE + 1] = {0};
    FIFO_MSG * response = NULL;

    struct sockaddr_in serv_addr;
    memset(&serv_addr, 0, sizeof(serv_addr)) ;
    serv_addr.sin_family = AF_INET;
    serv_addr.sin_addr.s_addr = inet_addr(SERVER_ADDR);
    serv_addr.sin_port = htons(SERVER_PORT);

    int server_sock_fd = socket(AF_INET, SOCK_STREAM, 0);
    if (server_sock_fd == -1) {
        printf("socket error");
        return -1;
    }

    if (connect(server_sock_fd, (struct sockaddr *)&serv_addr, sizeof(serv_addr)) != 0) {
        printf("connection error, %s, line %d.\n", strerror(errno), __LINE__);
        ret = -1;
        goto CLEAN;
    } else printf("client connected\n");


    if ((byte_num = send(server_sock_fd, reinterpret_cast<char *>(fiforequest), static_cast<int>(fiforequest_size), 0)) == -1) {
        printf("connection error, %s, line %d..\n", strerror(errno), __LINE__);
        ret = -1;
        goto CLEAN;
    } else printf("client send\n");

    byte_num = recv(server_sock_fd, reinterpret_cast<char *>(recv_msg), BUFFER_SIZE, 0);
    if (byte_num > 0) {
        if (byte_num > BUFFER_SIZE) {
            byte_num = BUFFER_SIZE;
        }

        recv_msg[byte_num] = '\0';

        response = (FIFO_MSG *)malloc((size_t)byte_num);
        if (!response) {
            printf("memory allocation failure.\n");
            return -1;
        }
        memset(response, 0, (size_t)byte_num);

        memcpy(response, recv_msg, (size_t)byte_num);

        *fiforesponse = response;
        *fiforesponse_size = (size_t)byte_num;
        printf("client received\n");
        ret = 0;
    } else if(byte_num < 0) {
        printf("server error, error message is %s!\n", strerror(errno));
        ret = -1;
    } else {
        printf("server exit!\n");
        ret = -1;
    }

CLEAN:
    close(server_sock_fd);

    return ret;
}
