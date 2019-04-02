#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <sys/types.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <spawn.h>

int main(int argc, const char *argv[]) {
	const int BUF_SIZE = 0x1000;
	const char* message = "Hello world!";
	int ret;

	if (argc != 2) {
		printf("usage: ./client <ipaddress>\n");
		return -1;
	}

	int sockfd = socket(AF_INET, SOCK_STREAM, 0);
	if (sockfd < 0) {
		printf("create socket error: %s(errno: %d)\n", strerror(errno), errno);
		return -1;
	}

	struct sockaddr_in servaddr;
	memset(&servaddr, 0, sizeof(servaddr));
	servaddr.sin_family = AF_INET;
	servaddr.sin_port = htons(6666);

	ret = inet_pton(AF_INET, argv[1], &servaddr.sin_addr);
	if (ret <= 0) {
		printf("inet_pton error for %s\n", argv[1]);
		return -1;
	}

	ret = connect(sockfd, (struct sockaddr *) &servaddr, sizeof(servaddr));
	if (ret < 0) {
		printf("connect error: %s(errno: %d)\n", strerror(errno), errno);
		return -1;
	}

	printf("send msg to server: %s\n", message);
	ret = send(sockfd, message, strlen(message), 0);
	if (ret < 0) {
		printf("send msg error: %s(errno: %d)\n", strerror(errno), errno);
		return -1;
	}

	close(sockfd);
	return 0;
}