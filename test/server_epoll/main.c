// Modified from https://banu.com/blog/2/how-to-use-epoll-a-complete-example-in-c/epoll-example.c
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <sys/socket.h>
#include <netdb.h>
#include <unistd.h>
#include <fcntl.h>
#include <sys/epoll.h>
#include <errno.h>
#include <spawn.h>

#define MAXEVENTS 64

static int
create_and_bind() {
	int listenfd = socket(AF_INET, SOCK_STREAM | SOCK_NONBLOCK, 0);
	if (listenfd < 0) {
		printf("create socket error: %s(errno: %d)\n", strerror(errno), errno);
		return -1;
	}

	struct sockaddr_in servaddr;
	memset(&servaddr, 0, sizeof(servaddr));
	servaddr.sin_family = AF_INET;
	servaddr.sin_addr.s_addr = htonl(INADDR_ANY);
	servaddr.sin_port = htons(6666);

	int ret = bind(listenfd, (struct sockaddr *) &servaddr, sizeof(servaddr));
	if (ret < 0) {
		printf("bind socket error: %s(errno: %d)\n", strerror(errno), errno);
		return -1;
	}
	return listenfd;
}

int
main(int argc, char *argv[]) {
	int sfd = create_and_bind();

	int s = listen(sfd, SOMAXCONN);
	if (s == -1) {
		perror("listen");
		return -1;
	}

	int efd = epoll_create1(0);
	if (efd == -1) {
		perror("epoll_create");
		return -1;
	}

	struct epoll_event event;
	event.data.fd = sfd;
	event.events = EPOLLIN | EPOLLET;
	s = epoll_ctl(efd, EPOLL_CTL_ADD, sfd, &event);
	if (s == -1) {
		perror("epoll_ctl");
		return -1;
	}

	/* Buffer where events are returned */
	struct epoll_event *events = calloc(MAXEVENTS, sizeof event);

	// spawn clients
	int client_pid;
	char* client_argv[] = {"client", "127.0.0.1"};
	for(int i=0; i<3; ++i) {
		int ret = posix_spawn(&client_pid, "client", NULL, NULL, client_argv, NULL);
		if (ret < 0) {
			printf("spawn client process error: %s(errno: %d)\n", strerror(errno), errno);
			return -1;
		}
	}

	/* The event loop */
	int done_count = 0;
	while (done_count < 3) {
		int n = epoll_wait(efd, events, MAXEVENTS, -1);
		for (int i = 0; i < n; i++) {
			if ((events[i].events & EPOLLERR) ||
				(events[i].events & EPOLLHUP) ||
				(!(events[i].events & EPOLLIN))) {
				/* An error has occured on this fd, or the socket is not
				   ready for reading (why were we notified then?) */
				fprintf(stderr, "epoll error\n");
				close(events[i].data.fd);
				continue;
			} else if (sfd == events[i].data.fd) {
				/* We have a notification on the listening socket, which
				   means one or more incoming connections. */
				while (1) {
					struct sockaddr in_addr;
					socklen_t in_len;
					int infd;
					char hbuf[NI_MAXHOST], sbuf[NI_MAXSERV];

					in_len = sizeof in_addr;
					infd = accept4(sfd, &in_addr, &in_len, SOCK_NONBLOCK);
					if (infd == -1) {
						if ((errno == EAGAIN) ||
							(errno == EWOULDBLOCK)) {
							/* We have processed all incoming
							   connections. */
							break;
						} else {
							perror("accept");
							break;
						}
					}

					s = getnameinfo(&in_addr, in_len,
									hbuf, sizeof hbuf,
									sbuf, sizeof sbuf,
									NI_NUMERICHOST | NI_NUMERICSERV);
					if (s == 0) {
						printf("Accepted connection on descriptor %d "
							   "(host=%s, port=%s)\n", infd, hbuf, sbuf);
					}

					// add it to the list of fds to monitor
					event.data.fd = infd;
					event.events = EPOLLIN | EPOLLET;
					s = epoll_ctl(efd, EPOLL_CTL_ADD, infd, &event);
					if (s == -1) {
						perror("epoll_ctl");
						return -1;
					}
				}
				continue;
			} else {
				/* We have data on the fd waiting to be read. Read and
				   display it. We must read whatever data is available
				   completely, as we are running in edge-triggered mode
				   and won't get a notification again for the same
				   data. */
				int done = 0;

				while (1) {
					ssize_t count;
					char buf[512];

					count = read(events[i].data.fd, buf, sizeof buf);
					if (count == -1) {
						/* If errno == EAGAIN, that means we have read all
						   data. So go back to the main loop. */
						if (errno != EAGAIN) {
							perror("read");
							done = 1;
						}
						break;
					} else if (count == 0) {
						/* End of file. The remote has closed the
						   connection. */
						done = 1;
						break;
					}

					/* Write the buffer to standard output */
					s = write(1, buf, count);
					if (s == -1) {
						perror("write");
						return -1;
					}
				}

				if (done) {
					printf("Closed connection on descriptor %d\n",
						   events[i].data.fd);

					/* Closing the descriptor will make epoll remove it
					   from the set of descriptors which are monitored. */
					close(events[i].data.fd);

					done_count ++;
				}
			}
		}
	}

	free(events);

	close(sfd);

	return EXIT_SUCCESS;
}
