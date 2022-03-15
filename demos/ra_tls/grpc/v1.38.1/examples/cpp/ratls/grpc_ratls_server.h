#ifndef _GRPC_RATLS_SERVER_H_
#define _GRPC_RATLS_SERVER_H_

#ifdef __cplusplus
extern "C" {
#endif

// start server
extern int grpc_ratls_start_server(
    const char *server_addr, // grpc server address+port, such as "localhost:50051"
    const char *config_json, // ratls handshake config json file
    const char *secret_json  // secret config json file
);

#ifdef __cplusplus
}
#endif

#endif  // _GRPC_RATLS_SERVER_H_