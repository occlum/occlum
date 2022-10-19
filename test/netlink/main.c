#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <sys/socket.h>
#include <unistd.h>
#include <linux/netlink.h>
#include <linux/rtnetlink.h>
#include "test.h"

// ============================================================================
// Helper macros
// ============================================================================

#define REPLY_BUFFER_LEN    256
#define REPLY_BUFFER_COUNT  8
#define TEST_USER_BUF_LEN   20

typedef struct nl_req nl_req_t;

struct nl_req {
    struct nlmsghdr hdr;
    struct rtgenmsg gen;
};

// ============================================================================
// Helper functions
// ============================================================================

void rtnl_print_link(struct nlmsghdr *h) {
    struct ifinfomsg *iface;
    struct rtattr *attribute;
    int len;

    iface = NLMSG_DATA(h);
    len = h->nlmsg_len - NLMSG_LENGTH(sizeof(*iface));

    /* loop over all attributes for the NEWLINK message */
    for (attribute = IFLA_RTA(iface); RTA_OK(attribute, len);
            attribute = RTA_NEXT(attribute, len)) {
        switch (attribute->rta_type) {
            case IFLA_IFNAME:
                printf("Interface %d : %s\n", iface->ifi_index, (char *) RTA_DATA(attribute));
                break;
            default:
                break;
        }
    }
}

void recv_and_parse_reply(int fd, struct sockaddr_nl *remote_addr,
                          socklen_t remote_addr_len) {
    int end = 0;                 /* some flag to end loop parsing */
    int len;
    struct nlmsghdr *msg_ptr;
    struct msghdr rtnl_reply;
    struct iovec io_reply;
    struct iovec iov[REPLY_BUFFER_COUNT];
    char reply[REPLY_BUFFER_COUNT][REPLY_BUFFER_LEN];

    while (!end) {
        memset(&io_reply, 0, sizeof(io_reply));
        memset(&rtnl_reply, 0, sizeof(rtnl_reply));

        // use iov for recv
        for (int i = 0; i < REPLY_BUFFER_COUNT; i++) {
            iov[i].iov_base = reply[i];
            iov[i].iov_len = REPLY_BUFFER_LEN;
        }
        rtnl_reply.msg_iov = iov;
        rtnl_reply.msg_iovlen = REPLY_BUFFER_COUNT;
        rtnl_reply.msg_name = remote_addr;
        rtnl_reply.msg_namelen = remote_addr_len;

        len = recvmsg(fd, &rtnl_reply, 0);
        if (len) {
            printf("start parsing\n");
            for (msg_ptr = (struct nlmsghdr *) reply; NLMSG_OK(msg_ptr, len);
                    msg_ptr = NLMSG_NEXT(msg_ptr, len)) {
                switch (msg_ptr->nlmsg_type) {
                    case NLMSG_DONE:
                        end++;
                        break;
                    case RTM_NEWLINK:
                        rtnl_print_link(msg_ptr);
                        break;
                    case RTM_NEWROUTE:
                        printf("Get route list\n");
                        break;
                    case RTM_NEWADDR:
                        printf("Get ip addr\n");
                        break;
                    default:
                        printf("Ignore unknown message type %d, length %d\n", msg_ptr->nlmsg_type,
                               msg_ptr->nlmsg_len);
                        break;
                }
            }
        }
    }
}

int create_netlink_socket_with_pid(pid_t pid) {
    int fd;
    struct sockaddr_nl local;       /* local addr */

    fd = socket(AF_NETLINK, SOCK_RAW, NETLINK_ROUTE);

    memset(&local, 0, sizeof(local));
    local.nl_family = AF_NETLINK;
    local.nl_pid = pid;
    local.nl_groups = 0;

    // bind local address
    if (bind(fd, (struct sockaddr *) &local, sizeof(local)) < 0) {
        THROW_ERROR("bind failure");
    }

    return fd;
}

// ============================================================================
// Test cases
// ============================================================================
int test_netlink_with_kernel() {
    pid_t pid = getpid();
    int fd = create_netlink_socket_with_pid(pid);
    if (fd < 0) {
        THROW_ERROR("netlink socket create failed");
    }

    // test default peername
    struct sockaddr_nl peer;
    socklen_t peer_len = sizeof(peer);
    if (getpeername(fd, (struct sockaddr *) &peer, &peer_len) < 0) {
        THROW_ERROR("getpeername() failed");
    }
    printf("Peer family: %d\n", peer.nl_family);
    printf("Peer port: %d\n", peer.nl_pid);
    printf("peer groups: %d\n", peer.nl_groups);
    if (peer.nl_pid != 0 || peer.nl_groups != 0) {
        THROW_ERROR("getpeername error");
    }

    struct sockaddr_nl kernel;      /* remote / kernel space addr */
    struct iovec iov;               /* IO vector for sendmsg */
    struct msghdr rtnl_msg;         /* generic msghdr struct for use with sendmsg */

    nl_req_t req;                   /* structure that describes the rtnetlink packet itself */
    memset(&rtnl_msg, 0, sizeof(rtnl_msg));
    memset(&kernel, 0, sizeof(kernel));
    memset(&req, 0, sizeof(req));

    // set remote address
    kernel.nl_family = AF_NETLINK;
    kernel.nl_pid = 0;
    kernel.nl_groups = 0;

    req.hdr.nlmsg_len = NLMSG_LENGTH(sizeof(struct rtgenmsg));
    req.hdr.nlmsg_type = RTM_GETLINK;
    req.hdr.nlmsg_flags = NLM_F_REQUEST | NLM_F_DUMP;
    req.hdr.nlmsg_seq = 1;
    req.hdr.nlmsg_pid = pid;
    req.gen.rtgen_family = AF_INET;

    iov.iov_base = &req;
    iov.iov_len = req.hdr.nlmsg_len;
    rtnl_msg.msg_iov = &iov;
    rtnl_msg.msg_iovlen = 1;
    rtnl_msg.msg_name = &kernel;
    rtnl_msg.msg_namelen = sizeof(kernel);

    if (sendmsg(fd, (struct msghdr *) &rtnl_msg, 0) < 0) {
        THROW_ERROR("sendmsg failure");
    }

    recv_and_parse_reply(fd, &kernel, sizeof(kernel));

    close(fd);

    return 0;
}

// This case can't pass on GitHub virtual machines. The write will fail with EPERM for unknown reason.
// Only run this case on self-hosted machine.
#ifdef SGX_MODE_HW
int test_netlink_between_user() {
    pid_t pid_1 = getpid() + 1;
    int sock_1 = create_netlink_socket_with_pid(pid_1);
    if (sock_1 < 0) {
        THROW_ERROR("netlink socket create failed");
    }

    pid_t pid_2 = pid_1 + 1;
    int sock_2 = socket(AF_NETLINK, SOCK_RAW, NETLINK_ROUTE);
    if (sock_2 < 0) {
        THROW_ERROR("netlink socket create failed");
    }

    struct sockaddr_nl sock_2_addr;
    memset(&sock_2_addr, 0, sizeof(sock_2_addr));
    sock_2_addr.nl_family = AF_NETLINK;
    sock_2_addr.nl_pid = pid_2;
    sock_2_addr.nl_groups = 0;
    if (bind(sock_2, (struct sockaddr *) &sock_2_addr, sizeof(sock_2_addr)) < 0) {
        THROW_ERROR("bind failure");
    }

    // sock_1 connect to sock_2
    int ret = connect(sock_1, (struct sockaddr *) &sock_2_addr, sizeof(sock_2_addr));
    if (ret < 0) {
        THROW_ERROR("connect to sock_2 failed");
    }

    // test getpeername
    struct sockaddr_nl peer;
    socklen_t peer_len = sizeof(peer);
    if (getpeername(sock_1, (struct sockaddr *) &peer, &peer_len) < 0) {
        THROW_ERROR("getpeername() failed");
    }
    printf("Peer family: %d\n", peer.nl_family);
    printf("Peer port: %d\n", peer.nl_pid);
    printf("peer groups: %d\n", peer.nl_groups);
    if (peer.nl_pid != sock_2_addr.nl_pid || peer.nl_groups != sock_2_addr.nl_groups) {
        THROW_ERROR("getpeername error");
    }

    char send_buf[TEST_USER_BUF_LEN] = "Hello netlink\n";
    char recv_buf[TEST_USER_BUF_LEN] = {0};

    // test write and read after connect
    ret = write(sock_1, send_buf, sizeof(send_buf));
    if (ret < 0) {
        THROW_ERROR("write to sock_2 failed");
    }

    int len = read(sock_2, recv_buf, sizeof(recv_buf));
    if (len < 0) {
        THROW_ERROR("recv failure");
    }

    printf("recv msg: %s\n", recv_buf);
    if (memcmp(send_buf, recv_buf, len) != 0) {
        THROW_ERROR("memcmp failure");
    }

    return 0;
}
#endif

// ============================================================================
// Test suite main
// ============================================================================
static test_case_t test_cases[] = {
    TEST_CASE(test_netlink_with_kernel),
#ifdef SGX_MODE_HW
    TEST_CASE(test_netlink_between_user),
#endif
};

int main() {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
