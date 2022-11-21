#include <stdio.h>
#include <stdlib.h>
#include <errno.h>
#include <string.h>
#include <sys/types.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>
#include "connection.h"

#define DATA "Hello from client"

static char *SERVER_PATH;

int client_run(void) {

    int client_sock, rc, len;
    struct sockaddr_un server_sockaddr;
    struct sockaddr_un client_sockaddr;
    char buf[256];
    memset(&server_sockaddr, 0, sizeof(struct sockaddr_un));
    memset(&client_sockaddr, 0, sizeof(struct sockaddr_un));


    client_sock = socket(AF_UNIX, SOCK_STREAM, 0);
    if (client_sock == -1) {
        printf("SOCKET ERROR = %d\n", errno);
        exit(1);
    }

    client_sockaddr.sun_family = AF_UNIX;
    strcpy(client_sockaddr.sun_path, CLIENT_PATH);
    len = strlen(client_sockaddr.sun_path) + sizeof(client_sockaddr.sun_family) + 1;

    unlink(CLIENT_PATH);
    rc = bind(client_sock, (struct sockaddr *) &client_sockaddr, len);
    if (rc == -1) {
        printf("BIND ERROR: %d\n", errno);
        close(client_sock);
        exit(1);
    }

    server_sockaddr.sun_family = AF_UNIX;
    strcpy(server_sockaddr.sun_path, SERVER_PATH);
    len = strlen(server_sockaddr.sun_path) + sizeof(server_sockaddr.sun_family) + 1;
    rc = connect(client_sock, (struct sockaddr *) &server_sockaddr, len);
    if (rc == -1) {
        printf("CONNECT ERROR = %d\n", errno);
        close(client_sock);
        exit(1);
    }

    strcpy(buf, DATA);
    printf("Sending data...\n");
    rc = send(client_sock, buf, strlen(buf), 0);
    if (rc == -1) {
        printf("SEND ERROR = %d\n", errno);
        close(client_sock);
        exit(1);
    } else {
        printf("Data sent!\n");
    }

    /**************************************/
    /* Read the data sent from the server */
    /* and print it.                      */
    /**************************************/
    printf("Waiting to recieve data...\n");
    memset(buf, 0, sizeof(buf));
    rc = recv(client_sock, buf, sizeof(buf), 0);
    if (rc == -1) {
        printf("RECV ERROR = %d\n", errno);
        close(client_sock);
        exit(1);
    } else {
        printf("DATA RECEIVED = %s\n", buf);
    }

    /******************************/
    /* Close the socket and exit. */
    /******************************/
    close(client_sock);

    return 0;
}

void print_usage() {
    fprintf(stderr, "Usage:\n ./client <trusted, untrusted, both_trusted, both_untrusted> \n\n");
}

int main(int argc, char **argv) {
    if (argc <= 1) {
        print_usage();
        return 1;
    }

    char *env = argv[1];
    if (strncmp(env, ENV_BU, sizeof(ENV_BU)) == 0) {
        // Both client and server running in host.
        // Client directly connects to the ready path.
        SERVER_PATH = SERVER_SOCK_READY_PATH_U_0;
    } else if (strncmp(env, ENV_U, sizeof(ENV_U)) == 0) {
        // Server running in libos, client running in host.
        // Client connects to the host path defined in Occlum.yaml untrusted_unix_socks.host
        SERVER_PATH = SERVER_SOCK_PATH_U_1;
    } else if (strncmp(env, ENV_T, sizeof(ENV_T)) == 0) {
        // Server running in host, client running in libos
        // Client connects to the same name in the corresponding directory of libos.
        SERVER_PATH = SERVER_SOCK_READY_PATH_T_2;
    } else if (strncmp(env, ENV_BT, sizeof(ENV_BT)) == 0) {
        // Both client and server running in libos but in different instances
        // Client connects to the libos path defined in Oclcum.json untrusted_unix_socks.libos
        SERVER_PATH = SERVER_SOCK_PATH_T_3;
    } else {
        print_usage();
        fprintf(stderr, "unknown enviroment");
        exit(1);
    }

    return client_run();
}
