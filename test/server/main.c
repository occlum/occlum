#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <sys/types.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <spawn.h>
#include <unistd.h>

int main(int argc, const char *argv[]) {
	const int BUF_SIZE = 0x1000;
	int ret;

	int listenfd = socket(AF_INET, SOCK_STREAM, 0);
	if (listenfd < 0) {
		printf("create socket error: %s(errno: %d)\n", strerror(errno), errno);
		return -1;
	}

	struct sockaddr_in servaddr;
	memset(&servaddr, 0, sizeof(servaddr));
	servaddr.sin_family = AF_INET;
	servaddr.sin_addr.s_addr = htonl(INADDR_ANY);
	servaddr.sin_port = htons(6666);

	ret = bind(listenfd, (struct sockaddr *) &servaddr, sizeof(servaddr));
	if (ret < 0) {
		printf("bind socket error: %s(errno: %d)\n", strerror(errno), errno);
		return -1;
	}

	ret = listen(listenfd, 10);
	if (ret < 0) {
		printf("listen socket error: %s(errno: %d)\n", strerror(errno), errno);
		return -1;
	}

	int client_pid;
	char* client_argv[] = {"client", "127.0.0.1", "6666", NULL};
	ret = posix_spawn(&client_pid, "/bin/client", NULL, NULL, client_argv, NULL);
	if (ret < 0) {
		printf("spawn client process error: %s(errno: %d)\n", strerror(errno), errno);
		return -1;
	}

	printf("====== waiting for client's request ======\n");
	int connect_fd = accept(listenfd, (struct sockaddr *) NULL, NULL);
	if (connect_fd < 0) {
		printf("accept socket error: %s(errno: %d)", strerror(errno), errno);
		return -1;
	}
	char buff[BUF_SIZE];
	int n = recv(connect_fd, buff, BUF_SIZE, 0);
	buff[n] = '\0';
	printf("recv msg from client: %s\n", buff);
	close(connect_fd);

	close(listenfd);
}
