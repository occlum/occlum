#include <sys/time.h>
#include <sys/types.h>
#include <sys/select.h>
#include <sys/socket.h>
#include <errno.h>
#include <stdio.h>
#include <stddef.h>
#include "ocalls.h"

ssize_t occlum_ocall_sendmsg(int sockfd,
                             const void *msg_name,
                             socklen_t msg_namelen,
                             const struct iovec *msg_iov,
                             size_t msg_iovlen,
                             const void *msg_control,
                             size_t msg_controllen,
                             int flags) {
    struct msghdr msg = {
        (void *) msg_name,
        msg_namelen,
        (struct iovec *) msg_iov,
        msg_iovlen,
        (void *) msg_control,
        msg_controllen,
        0,
    };
    return sendmsg(sockfd, &msg, flags);
}

ssize_t occlum_ocall_recvmsg(int sockfd,
                             void *msg_name,
                             socklen_t msg_namelen,
                             socklen_t *msg_namelen_recv,
                             struct iovec *msg_iov,
                             size_t msg_iovlen,
                             void *msg_control,
                             size_t msg_controllen,
                             size_t *msg_controllen_recv,
                             int *msg_flags_recv,
                             int flags) {
    struct msghdr msg = {
        msg_name,
        msg_namelen,
        msg_iov,
        msg_iovlen,
        msg_control,
        msg_controllen,
        0,
    };
    ssize_t ret = recvmsg(sockfd, &msg, flags);
    if (ret < 0) { return ret; }

    *msg_namelen_recv = msg.msg_namelen;
    *msg_controllen_recv = msg.msg_controllen;
    *msg_flags_recv = msg.msg_flags;
    return ret;
}

int occlum_ocall_poll(struct pollfd *fds,
                      nfds_t nfds,
                      struct timeval *timeout,
                      int efd) {
    struct timeval start_tv, end_tv, elapsed_tv;
    int real_timeout = (timeout == NULL) ? -1 :
                       (timeout->tv_sec * 1000 + timeout->tv_usec / 1000);
    if (timeout != NULL) {
        gettimeofday(&start_tv, NULL);
    }

    int ret = poll(fds, nfds, real_timeout);

    if (timeout != NULL) {
        gettimeofday(&end_tv, NULL);
        timersub(&end_tv, &start_tv, &elapsed_tv);
        if timercmp(timeout, &elapsed_tv, >= ) {
            timersub(timeout, &elapsed_tv, timeout);
        } else {
            timeout->tv_sec = 0;
            timeout->tv_usec = 0;
        }
    }

    int saved_errno = errno;
    // clear the status of the eventfd
    uint64_t u = 0;
    read(efd, &u, sizeof(uint64_t));
    // restore the errno of poll
    errno = saved_errno;
    return ret;
}
