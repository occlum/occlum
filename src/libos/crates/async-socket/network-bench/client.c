#include <netdb.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <sys/socket.h>
#include <sys/time.h>
#include <unistd.h>
#include <pthread.h>
#include <netinet/tcp.h>


#define SERVER_NAME_LEN_MAX 255

typedef struct pthread_arg_t {
    int socket_fd;
    int block_size;
    long req_num;
    double duration;
} pthread_arg_t;

void *pthread_routine(void *arg);

int main(int argc, char *argv[]) {
    char server_name[SERVER_NAME_LEN_MAX + 1] = { 0 };
    int server_port, socket_fd, block_size, client_num, req_num;
    struct hostent *server_host;
    struct sockaddr_in server_address;

    if (argc > 1) {
        strncpy(server_name, argv[1], SERVER_NAME_LEN_MAX);
    } else {
        printf("Enter Server Name: ");
        int ret = scanf("%s", server_name);
    }

    server_port = argc > 2 ? atoi(argv[2]) : 0;
    if (!server_port) {
        printf("Enter Port: ");
        int ret = scanf("%d", &server_port);
    }

    block_size = argc > 3 ? atoi(argv[3]) : 0;
    if (!block_size) {
        printf("Enter Block Size: ");
        int ret = scanf("%d", &block_size);
    }

    client_num = argc > 4 ? atoi(argv[4]) : 0;
    if (!client_num) {
        printf("Enter Client Num: ");
        int ret = scanf("%d", &client_num);
    }

    req_num = argc > 5 ? atoi(argv[5]) : 0;
    if (!req_num) {
        printf("Enter Request Num: ");
        int ret = scanf("%d", &req_num);
    }

    pthread_t* tid = (pthread_t*)malloc(client_num * sizeof(pthread_t));
    pthread_arg_t* pthread_args = (pthread_arg_t*)malloc(client_num * sizeof(pthread_arg_t));

    for (int i = 0; i < client_num; i++) {
        server_host = gethostbyname(server_name);

        memset(&server_address, 0, sizeof server_address);
        server_address.sin_family = AF_INET;
        server_address.sin_port = htons(server_port);
        memcpy(&server_address.sin_addr.s_addr, server_host->h_addr, server_host->h_length);

        if ((socket_fd = socket(AF_INET, SOCK_STREAM, 0)) == -1) {
            perror("[client] socket");
            exit(1);
        }

        if (connect(socket_fd, (struct sockaddr *)&server_address, sizeof server_address) == -1) {
	    	perror("[client] connect");
            exit(1);
	    }

        pthread_args[i].block_size = block_size;
        pthread_args[i].req_num = req_num;
        pthread_args[i].socket_fd = socket_fd;
        pthread_args[i].duration = 0;
    }

    struct timeval tv1;
    gettimeofday(&tv1, NULL);

    for (int i = 0; i < client_num; i++) {
        if (pthread_create(&tid[i], NULL, pthread_routine, (void *)&pthread_args[i]) != 0) {
            perror("[client] pthread_create");
            continue;
        }
    }

    double avg_time = 0;
    for (int i = 0; i < client_num; i++) {
       pthread_join(tid[i], NULL);
       avg_time += pthread_args[i].duration;
    }
    avg_time /= client_num;

    struct timeval tv2;
    gettimeofday(&tv2, NULL);
    double duration = (double) (tv2.tv_usec - tv1.tv_usec) / 1000000 + (double) (tv2.tv_sec - tv1.tv_sec);

    int send_size = (long) block_size * req_num * client_num / 1024 / 1024;
    double throughput = send_size * 2 / duration;
    double avg_throughput = send_size * 2 / avg_time;
    printf("[client] client_num: %d, block_size: %d, request_num: %d, send_size: %d MB, duration: %f s (avg_time: %f s), throughput: %f MB/s (avg_throughput: %f MB/s)\n",
        client_num, block_size, req_num, send_size, duration, avg_time, throughput, avg_throughput);

    free(tid);
    free(pthread_args);

    return 0;
}

void *pthread_routine(void *arg) {
    struct timeval tv1;
    gettimeofday(&tv1, NULL);

    pthread_arg_t *pthread_arg = (pthread_arg_t *)arg;
    int socket_fd = pthread_arg->socket_fd;
    int block_size = pthread_arg->block_size;
    int req_num = pthread_arg->req_num;

    void* buf = malloc(block_size);
    int cnt = 0;
    int dup = 0;
    while (1) {
        int bytes_write = write(socket_fd, buf, block_size);
        if (bytes_write != block_size) {
            printf("[client] bytes_write != block_size, %d, %d\n", bytes_write, block_size);
            break;
        }

        int bytes_read = read(socket_fd, buf, block_size);
        if (bytes_read < 0) {
            perror("[client] read");
            break;
        } else if (bytes_read < block_size) {
            int bytes_read2 = read(socket_fd, buf + bytes_read, block_size - bytes_read);
            if (bytes_read2 + bytes_read != block_size) {
                printf("[client] bytes_read != block_size, %d, %d, %d\n", bytes_read, bytes_read2, block_size);
                break;
            }
            dup++;
        }

        cnt++;
        if (cnt == req_num) break;
    }

    if (dup > req_num / 10) printf("retry read number: %d\n", dup);

    close(socket_fd);

    struct timeval tv2;
    gettimeofday(&tv2, NULL);
    pthread_arg->duration = (double) (tv2.tv_usec - tv1.tv_usec) / 1000000 + (double) (tv2.tv_sec - tv1.tv_sec);
}