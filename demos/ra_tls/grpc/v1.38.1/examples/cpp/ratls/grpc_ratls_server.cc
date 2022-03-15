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

#include <grpcpp/grpcpp.h>
#include <grpcpp/security/sgx/sgx_ra_tls.h>
#include <grpcpp/ext/proto_server_reflection_plugin.h>

#ifdef BAZEL_BUILD
#include "examples/protos/ratls.grpc.pb.h"
#else
#include "ratls.grpc.pb.h"
#endif

#include "../grpc_ratls_server.h"

using ratls::GrSecret;
using ratls::SecretRequest;
using ratls::SecretReply;


// Logic and data behind the server's behavior.
class GrSecretServiceImpl final: public GrSecret::Service {
    public:
        grpc::Status GetSecret(
            grpc::ServerContext* context, const SecretRequest* request, SecretReply* reply) override {
            //std::cout << "Request:  " << request->name() << std::endl;
            auto secret = this->get_secret_string(request->name().c_str());
            if (!secret.empty()) {
                reply->set_secret(secret);
                return grpc::Status::OK;
            } else {
                return grpc::Status::CANCELLED;
            }
        }

        GrSecretServiceImpl(const char* file) : secret_file(nullptr) {
            this->secret_file = file;
        }

    private:
        std::string get_secret_string(const char *name) {
            std::string secret = "";
            class grpc::sgx::json_engine secret_config(this->secret_file);
            auto item = secret_config.get_item(secret_config.get_handle(), name);
            if (item) {
                secret = secret_config.print_item(item);
            }

            return secret;
        }

        const char *secret_file;
};


int grpc_ratls_start_server(
    const char *server_addr,
    const char *config_json,
    const char *secret_json
) 
{
    GrSecretServiceImpl service(secret_json);

    grpc::EnableDefaultHealthCheckService(true);
    grpc::reflection::InitProtoReflectionServerBuilderPlugin();
    grpc::ServerBuilder builder;

    auto creds = grpc::sgx::TlsServerCredentials(config_json);
    GPR_ASSERT(creds.get() != nullptr);

    builder.AddListeningPort(server_addr, creds);
    builder.RegisterService(&service);

    std::unique_ptr<grpc::Server> server(builder.BuildAndStart());
    std::cout << "Server listening on " << server_addr << std::endl;

    server->Wait();

    return 0;
}

