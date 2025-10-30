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

#define MAX_PACKET_SIZE 4096

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

    /* Bind an IP addr to send and recv */
    struct sockaddr_in addr;
    memset(&addr, 0, sizeof(addr));
    addr.sin_family = AF_INET;
    addr.sin_port = htons(8808);
    addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    if (bind(sockfd, (struct sockaddr *)&addr, sizeof(addr)) < 0) {
        close(sockfd);
        THROW_ERROR("bind failed");
    }

    /* Send an IP packet with TCP header */
    char packet[MAX_PACKET_SIZE];
    memset(packet, 0, MAX_PACKET_SIZE);
    struct tcphdr *tcp_header = (struct tcphdr *)(packet);
    tcp_header->th_sport = htons(8801);
    tcp_header->th_dport = htons(8808);
    tcp_header->th_seq = htonl(1);
    tcp_header->th_ack = 0;
    tcp_header->th_off = 5;
    tcp_header->th_flags = TH_SYN;
    tcp_header->th_win = htons(65535);
    tcp_header->th_sum = 0;
    tcp_header->th_urp = 0;
    char snd_data[] = "Hello from send!";
    memcpy(packet + sizeof(struct tcphdr), snd_data, strlen(snd_data));
    ssize_t send_size = sendto(sockfd, packet, sizeof(struct tcphdr) + strlen(snd_data), 0,
                               (struct sockaddr *)&addr, sizeof(struct sockaddr_in));
    if (send_size <= 0) {
        close(sockfd);
        THROW_ERROR("sendto failed");
    } else {
        printf("Send an IP packet with %ld bytes data\n", send_size);
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

    /* Build an ethernet frame, an ICMP packet */
    unsigned char eth_frame[98] = {
        /* Ethernet header */
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, /* dst MAC */
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, /* src MAC */
        0x08, 0x00,
        /* IPv4 header */
        0x45, 0x00, 0x00, 0x54,
        0x08, 0x31, 0x40, 0x00,
        0x40, 0x01, 0x34, 0x76,
        0x7f, 0x00, 0x00, 0x01,     /* src IP */
        0x7f, 0x00, 0x00, 0x01,     /* dst IP */
        /* ICMP header + payload */
        0x08, 0x00, 0xb6, 0xcf, 0x00, 0x05, 0x00, 0x01,
        0xdf, 0x7f, 0xe2, 0x67, 0x00, 0x00, 0x00, 0x00,
        0xba, 0x6f, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17,
        0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
        0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27,
        0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f,
        0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37
    };

    /* Send an Ethernet Frame */
    ssize_t snd_packet_len = send(sockfd, eth_frame, 98, 0);
    if (snd_packet_len <= 0) {
        close(sockfd);
        THROW_ERROR("sendto error");
    }
    printf("Send an Ethernet Frame with %ld bytes data\n", snd_packet_len);

    struct msghdr msg = {0};
    struct iovec iov;
    iov.iov_base = buffer;
    iov.iov_len = MAX_PACKET_SIZE;
    msg.msg_name = NULL;
    msg.msg_namelen = 0;
    msg.msg_iov = &iov;
    msg.msg_iovlen = 1;
    msg.msg_control = NULL;
    msg.msg_controllen = 0;
    msg.msg_flags = 0;

    /* Receive an Ethernet Frame */
    ssize_t packet_len = recvmsg(sockfd, &msg, 0);
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
