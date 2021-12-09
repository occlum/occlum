#include <stdio.h>
#include <stdlib.h>
#include <errno.h>
#include <string.h>
#include <sys/types.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>
#include "connection.h"

#define DATA "Hello from server"

static char *SOCK_PATH, *SOCK_READY_PATH;

int server_run(void) {
    int server_sock, client_sock, len, rc;
    int bytes_rec = 0;
    struct sockaddr_un server_sockaddr;
    struct sockaddr_un client_sockaddr;
    char buf[256];
    int backlog = 10;
    memset(&server_sockaddr, 0, sizeof(struct sockaddr_un));
    memset(&client_sockaddr, 0, sizeof(struct sockaddr_un));
    memset(buf, 0, 256);

    server_sock = socket(AF_UNIX, SOCK_STREAM, 0);
    if (server_sock == -1) {
        printf("SOCKET ERROR: %d\n", errno);
        exit(1);
    }

    server_sockaddr.sun_family = AF_UNIX;
    strcpy(server_sockaddr.sun_path, SOCK_PATH);
    len = sizeof(server_sockaddr);

    unlink(SOCK_PATH);
    printf("bind path = %s\n", SOCK_PATH);
    rc = bind(server_sock, (struct sockaddr *) &server_sockaddr, len);
    if (rc == -1) {
        printf("BIND ERROR: %d\n", errno);
        close(server_sock);
        exit(1);
    }

    rc = listen(server_sock, backlog);
    if (rc == -1) {
        printf("LISTEN ERROR: %d\n", errno);
        close(server_sock);
        exit(1);
    }

    // Rename to new path
    unlink(SOCK_READY_PATH);
    if (rename(SOCK_PATH, SOCK_READY_PATH) < 0) {
        printf("failed to rename");
        exit(1);
    }
    printf("socket listening...\n");

    len = sizeof(client_sockaddr);
    client_sock = accept(server_sock, (struct sockaddr *) &client_sockaddr, &len);
    if (client_sock == -1) {
        printf("ACCEPT ERROR: %d\n", errno);
        close(server_sock);
        close(client_sock);
        exit(1);
    }
    printf("Connected socket path: %s\n", client_sockaddr.sun_path);

    memset(&client_sockaddr, 0, sizeof(struct sockaddr_un));
    rc = getpeername(client_sock, (struct sockaddr *) &client_sockaddr, &len);
    if (rc == -1) {
        printf("GETPEERNAME ERROR: %d\n", errno);
        close(server_sock);
        close(client_sock);
        exit(1);
    } else {
        printf("Client socket filepath: %s\n", client_sockaddr.sun_path);
    }

    printf("waiting to read...\n");
    bytes_rec = recv(client_sock, buf, sizeof(buf), 0);
    if (bytes_rec == -1) {
        printf("RECV ERROR: %d\n", errno);
        close(server_sock);
        close(client_sock);
        exit(1);
    } else {
        printf("DATA RECEIVED = %s\n", buf);
    }

    memset(buf, 0, 256);
    strcpy(buf, DATA);
    printf("Sending data...\n");
    rc = send(client_sock, buf, strlen(buf), 0);
    if (rc == -1) {
        printf("SEND ERROR: %d", errno);
        close(server_sock);
        close(client_sock);
        exit(1);
    } else {
        printf("Data sent!\n");
    }

    close(server_sock);
    close(client_sock);

    unlink(SOCK_READY_PATH);

    return 0;
}

void print_usage() {
    fprintf(stderr, "Usage:\n ./server <trusted, untrusted, both_trusted, both_untrusted>\n\n");
}

int main(int argc, char **argv) {
    if (argc <= 1) {
        print_usage();
        return 1;
    }

    char *env = argv[1];
    if (strncmp(env, ENV_BU, sizeof(ENV_BU)) == 0) {
        // Both client and server running in host.
        SOCK_PATH = SERVER_SOCK_PATH_U_0;
        SOCK_READY_PATH = SERVER_SOCK_READY_PATH_U_0;
    } else if (strncmp(env, ENV_T, sizeof(ENV_T)) == 0) {
        // Server running in libos, client running in host.
        SOCK_PATH = SERVER_SOCK_PATH_T_1;
        SOCK_READY_PATH = SERVER_SOCK_READY_PATH_T_1;
    } else if (strncmp(env, ENV_U, sizeof(ENV_U)) == 0) {
        // Server running in host, client running in libos
        SOCK_PATH = SERVER_SOCK_PATH_U_2;
        SOCK_READY_PATH = SERVER_SOCK_READY_PATH_U_2;
    } else if (strncmp(env, ENV_BT, sizeof(ENV_BT)) == 0) {
        // Both client and server running in libos but in different instances
        SOCK_PATH = SERVER_SOCK_PATH_T_3;
        SOCK_READY_PATH = SERVER_SOCK_READY_PATH_T_3;
    } else {
        print_usage();
        fprintf(stderr, "unknown enviroment");
        exit(1);
    }

    return server_run();
}
