#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/socket.h>
#include <sys/ioctl.h>
#include <sys/types.h>
#include <linux/netlink.h>
#include <linux/rtnetlink.h>
#include <netinet/in.h>
#include <netinet/ip.h>
#include <netinet/tcp.h>
#include <netinet/ether.h>
#include <arpa/inet.h>
#include <net/if.h>
#include <netpacket/packet.h>
#include <fcntl.h>
#include "test.h"

// ============================================================================
// Test cases for raw socket with netlink message
// ============================================================================

#define BUFSIZE 8192

struct nlreq {
    struct nlmsghdr hdr;
    struct rtmsg msg;
};

int test_netlink_socket() {
    /* Create a netlink socket */
    int sockfd = socket(AF_NETLINK, SOCK_RAW, NETLINK_ROUTE);
    if (sockfd < 0) {
        THROW_ERROR("socket(AF_NETLINK, SOCK_RAW, NETLINK_ROUTE) failed");
    }

    /* Use socket to bind an address */
    struct sockaddr_nl sa;
    memset(&sa, 0, sizeof(sa));
    sa.nl_family = AF_NETLINK;
    if (bind(sockfd, (struct sockaddr *) &sa, sizeof(sa)) == -1) {
        close(sockfd);
        THROW_ERROR("bind failed");
    }

    /* Let's build a netlink request */
    struct nlreq req;
    memset(&req, 0, sizeof(req));
    req.hdr.nlmsg_len = NLMSG_LENGTH(sizeof(struct rtmsg));
    req.hdr.nlmsg_type = RTM_GETROUTE;
    req.hdr.nlmsg_flags = NLM_F_REQUEST | NLM_F_DUMP;
    req.msg.rtm_family = AF_INET;
    req.msg.rtm_table = RT_TABLE_MAIN;
    req.msg.rtm_protocol = RTPROT_UNSPEC;
    req.msg.rtm_scope = RT_SCOPE_UNIVERSE;
    req.msg.rtm_type = RTN_UNICAST;

    struct iovec iov;
    memset(&iov, 0, sizeof(iov));
    iov.iov_base = &req;
    iov.iov_len = req.hdr.nlmsg_len;

    struct msghdr msg;
    memset(&msg, 0, sizeof(msg));
    msg.msg_name = &sa;
    msg.msg_namelen = sizeof(sa);
    msg.msg_iov = &iov;
    msg.msg_iovlen = 1;

    /* Send the netlink message to kernel */
    if (sendmsg(sockfd, &msg, 0) == -1) {
        close(sockfd);
        THROW_ERROR("netlink sendmsg error");
    }

    /* Recv netlink message */
    char buf[BUFSIZE];
    struct nlmsghdr *hdr;
    int len;
    int nlmsg_num = 0;
    char gw_addr[INET_ADDRSTRLEN];
    memset(buf, 0, BUFSIZE);
    memset(gw_addr, 0, INET_ADDRSTRLEN);
    while ((len = recv(sockfd, buf, sizeof(buf), 0)) > 0) {
        printf("Receive %d bytes from kernel\n", len);
        /* Parse received message */
        for (hdr = (struct nlmsghdr *) buf; NLMSG_OK(hdr, len); hdr = NLMSG_NEXT(hdr, len)) {
            if (hdr->nlmsg_type == NLMSG_DONE) {
                ++nlmsg_num;
                goto finish;
            }
            if (hdr->nlmsg_type == NLMSG_ERROR) {
                close(sockfd);
                THROW_ERROR("received nl_msg error");
            }
            ++nlmsg_num;
            struct rtmsg *rt = (struct rtmsg *) NLMSG_DATA(hdr);
            if (rt->rtm_family != AF_INET || rt->rtm_table != RT_TABLE_MAIN ||
                    rt->rtm_type != RTN_UNICAST) {
                continue;
            }
            /* Get gateway address */
            struct rtattr *attr;
            int attrlen;
            for (attr = (struct rtattr *) RTM_RTA(rt), attrlen = RTM_PAYLOAD(hdr);
                    RTA_OK(attr, attrlen); attr = RTA_NEXT(attr, attrlen)) {
                if (attr->rta_type == RTA_GATEWAY) {
                    struct in_addr addr;
                    memcpy(&addr, RTA_DATA(attr), sizeof(addr));
                    if (inet_ntop(AF_INET, &addr, gw_addr, sizeof(gw_addr)) == NULL) {
                        close(sockfd);
                        THROW_ERROR("inet_ntop error");
                        continue;
                    }
                }
            }
        }
    }
    /* If the code reaching here, the recv() return value <= 0 */
    close(sockfd);
    THROW_ERROR("recv failed");
finish:
    close(sockfd);
    printf("Total nl_msg num: %d\n", nlmsg_num);
    printf("Gateway address: %s\n", gw_addr);
    return 0;
}

// ============================================================================
// Test cases for raw socket with ip packet
// ============================================================================

#define MAX_PACKET_SIZE 65536

/* Parse the TCP data within an IP packet */
int parse_packet(char *buffer, ssize_t data_size) {
    struct ip *ip_header = (struct ip *) buffer;
    int ip_header_len = ip_header->ip_hl * 4;

    /* Parse IP address */
    char *src_ip = inet_ntoa(ip_header->ip_src);
    char *dst_ip = inet_ntoa(ip_header->ip_dst);
    /* Parse TCP port */
    int src_port = 0;
    int dst_port = 0;
    struct tcphdr *tcp_header;
    if (ip_header->ip_p == IPPROTO_TCP) {
        tcp_header = (struct tcphdr *)(buffer + ip_header_len);
        src_port = ntohs(tcp_header->th_sport);
        dst_port = ntohs(tcp_header->th_dport);
    } else {
        /* Something wrong */
        return -1;
    }
    printf("Receive an IP packet with %ld bytes data\n", data_size);
    printf("From %s:%d to %s:%d\n", src_ip, src_port, dst_ip, dst_port);
    return 0;
}

int test_ip_socket() {
    /* Create a raw socket to acquire IP packet */
    int sockfd = socket(AF_INET, SOCK_RAW, IPPROTO_TCP);
    if (sockfd < 0) {
        close(sockfd);
        THROW_ERROR("socket(AF_INET, SOCK_RAW, IPPROTO_TCP) failed");
    }

    /* Receive an IP packet and parse TCP data within it */
    char buf[MAX_PACKET_SIZE];
    ssize_t data_size;
    memset(buf, 0, MAX_PACKET_SIZE);
    data_size = recv(sockfd, buf, MAX_PACKET_SIZE, 0);
    if (data_size < 0) {
        close(sockfd);
        THROW_ERROR("recv failed");
    }
    if (parse_packet(buf, data_size) < 0) {
        close(sockfd);
        THROW_ERROR("parse tcp data failed");
    }
    close(sockfd);
    return 0;
}

// ============================================================================
// Test cases for raw socket with raw packet
// ============================================================================

int test_packet_socket() {
    struct sockaddr_ll sa;
    unsigned char buffer[MAX_PACKET_SIZE];
    struct ifreq ifr;

    /* Create a packet socket to acquire raw packet */
    int sockfd = socket(AF_PACKET, SOCK_RAW, htons(ETH_P_ALL));
    if (sockfd < 0) {
        THROW_ERROR("socket(AF_PACKET, SOCK_RAW, htons(ETH_P_ALL) failed");
    }
    /* Acquire lo interface index */
    strncpy(ifr.ifr_name, "lo", IFNAMSIZ - 1);
    ifr.ifr_name[IFNAMSIZ - 1] = '\0';
    if (ioctl(sockfd, SIOCGIFINDEX, &ifr) == -1) {
        perror("ioctl(SIOCGIFINDEX) failed");
        close(sockfd);
        exit(EXIT_FAILURE);
    }
    memset(&sa, 0, sizeof(struct sockaddr_ll));
    sa.sll_family = AF_PACKET;
    sa.sll_protocol = htons(ETH_P_ALL);
    sa.sll_ifindex = ifr.ifr_ifindex;

    /* Bind the address with packet socket */
    if (bind(sockfd, (struct sockaddr *)&sa, sizeof(struct sockaddr_ll)) == -1) {
        close(sockfd);
        THROW_ERROR("bind error");
    }
    /* Receive an Ethernet Frame */
    ssize_t packet_len = recvfrom(sockfd, buffer, MAX_PACKET_SIZE, 0, NULL, NULL);
    if (packet_len <= 0) {
        close(sockfd);
        THROW_ERROR("recvfrom error");
    }
    struct ethhdr *eth_header = (struct ethhdr *)buffer;
    char *eth_src = ether_ntoa((struct ether_addr *)&eth_header->h_source);
    char *eth_dst = ether_ntoa((struct ether_addr *)&eth_header->h_dest);
    printf("Receive an Ethernet Frame with %ld bytes data\n", packet_len);
    printf("From %s to %s\n", eth_src, eth_dst);
    close(sockfd);
    return 0;
}

// ============================================================================
// Test suite
// ============================================================================

static test_case_t test_cases[] = {
    TEST_CASE(test_netlink_socket),
    TEST_CASE(test_ip_socket),
    TEST_CASE(test_packet_socket),
};

int main() {
    return test_suite_run(test_cases, ARRAY_SIZE(test_cases));
}
