#ifndef CONNECTION_H
#define CONNECTION_H

#define ENV_T "trusted"
#define ENV_U "untrusted"
#define ENV_BT "both_trusted"
#define ENV_BU "both_untrusted"

// Test both server and client running in host
#define SERVER_SOCK_PATH_U_0 "test.sock"
#define SERVER_SOCK_READY_PATH_U_0 ".test.sock"

// Test server running in libos, client running in host
// Corresponding Occlum.yaml config:
// untrusted_unix_socks:
//   - host: /tmp/occlum/test.sock
//     libos: /root/test.sock

#define SERVER_SOCK_PATH_T_1 "/root/test.sock"
#define SERVER_SOCK_READY_PATH_T_1 "/root/.test.sock"
#define SERVER_SOCK_PATH_U_1 "/tmp/occlum/test.sock"

// Test server running in host, client running in libos
// Corresponding Occlum.yaml config:
// untrusted_unix_socks:
//   - host: /tmp/root/
//     libos: /root/
#define SERVER_SOCK_PATH_U_2 "/tmp/root/test-2.sock"
#define SERVER_SOCK_READY_PATH_U_2 "/tmp/root/.test-2.sock"
#define SERVER_SOCK_READY_PATH_T_2 "/root/.test-2.sock"

// Test both server and client running in libos but in different Occlum instances
// Corresponding Occlum.yaml config:
// untrusted_unix_socks:
//   - host: ../test.sock
//     libos: /tmp/test.sock
#define SERVER_SOCK_PATH_T_3 "/tmp/test.sock"
#define SERVER_SOCK_READY_PATH_T_3 "/tmp/.test.sock"

// Client bind address
#define CLIENT_PATH "/tmp/client.sock"

#endif
