/*
 *
 * Copyright (c) 2022 Intel Corporation
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 *
 */
#include <stdio.h>
#include <string.h>
#include <iostream>
#include <fstream>

#include <grpcpp/grpcpp.h>
#include <grpcpp/security/sgx/sgx_ra_tls.h>

#ifdef BAZEL_BUILD
#include "examples/protos/ratls.grpc.pb.h"
#else
#include "ratls.grpc.pb.h"
#endif

#include "../grpc_ratls_client.h"

using ratls::GrSecret;
using ratls::SecretRequest;
using ratls::SecretReply;

// Client
class GrSecretClient {
    public:
        GrSecretClient(std::shared_ptr<grpc::Channel> channel) : stub_(GrSecret::NewStub(channel)) {}

        std::string GetSecret(const std::string& name) {
            SecretRequest request;
            request.set_name(name);

            SecretReply reply;

            grpc::ClientContext context;

            grpc::Status status = stub_->GetSecret(&context, request, &reply);

            if (status.ok()) {
                return reply.secret();
            } else {
                std::cout << status.error_code() << ": " << status.error_message() << std::endl;
                return "RPC failed";
            }
        }

    private:
        std::unique_ptr<GrSecret::Stub> stub_;
};

static const unsigned char base64_table[65] =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

static size_t base64_decode_len(const char *b64input) {
    size_t len = strlen(b64input), padding = 0;

    if (b64input[len - 1] == '=' && b64input[len - 2] == '=') { //last two chars are =
        padding = 2;
    } else if (b64input[len - 1] == '=') { //last char is =
        padding = 1;
    }

    return (len * 3) / 4 - padding;
}

/**
 * base64_decode - Base64 decode
 */
void base64_decode(const char *b64input, unsigned char *dest, size_t dest_len) {
    unsigned char dtable[256], *pos, block[4], tmp;
    size_t i, count, olen;
    size_t len = strlen(b64input);

    memset(dtable, 0x80, 256);
    for (i = 0; i < sizeof(base64_table) - 1; i++) {
        dtable[base64_table[i]] = (unsigned char) i;
    }
    dtable['='] = 0;

    olen = base64_decode_len(b64input);
    if (olen > dest_len) {
        printf("Base64 encoded length %ld is biggeer than %ld\n", olen, dest_len);
        return;
    }

    pos = dest;
    count = 0;
    for (i = 0; i < len; i++) {
        tmp = dtable[(unsigned char)b64input[i]];
        if (tmp == 0x80) {
            continue;
        }
        block[count] = tmp;
        count++;
        if (count == 4) {
            *pos++ = (block[0] << 2) | (block[1] >> 4);
            *pos++ = (block[1] << 4) | (block[2] >> 2);
            *pos++ = (block[2] << 6) | block[3];
            count = 0;
        }
    }
}

int grpc_ratls_get_secret(
    const char *server_addr,
    const char *config_json,
    const char *name,
    const char *secret_file
)
{
    auto cred = grpc::sgx::TlsCredentials(config_json);
    auto channel = grpc::CreateChannel(server_addr, cred);

    GrSecretClient gr_secret(channel);

    std::string secret = gr_secret.GetSecret(name);
    //std::cout << "secret received: " << secret << std::endl;

    //Decode From Base64
    size_t len = base64_decode_len(secret.c_str());
    if (len) {
        char *secret_orig = (char *)malloc(len);
        base64_decode(secret.c_str(), (unsigned char *)secret_orig, len);
        std::string secret_string(secret_orig, secret_orig + len - 1);

        //write to file
        std::ofstream myfile;
        myfile.open(secret_file);
        myfile << secret_string;
        myfile.close();
    }

    return 0;
}
