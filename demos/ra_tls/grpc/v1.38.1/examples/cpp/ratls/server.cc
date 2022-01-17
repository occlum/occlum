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

#include "../getopt.hpp"

using ratls::Greeter;
using ratls::HelloReply;
using ratls::HelloRequest;

struct argparser {
    const char* config;
    std::string server_address;
    argparser() {
        server_address = getarg("localhost:50051", "-host", "--host");
        config = getarg("dynamic_config.json", "-cfg", "--config");
    };
};

// Logic and data behind the server's behavior.
class GreeterServiceImpl final : public Greeter::Service {
    grpc::Status SayHello(
        grpc::ServerContext* context, const HelloRequest* request, HelloReply* reply) override {
        std::string prefix("Hello ");
        reply->set_message(prefix + request->name());
        return grpc::Status::OK;
    }
};

void RunServer() {
    argparser args;

    GreeterServiceImpl service;

    grpc::EnableDefaultHealthCheckService(true);
    grpc::reflection::InitProtoReflectionServerBuilderPlugin();

    grpc::ServerBuilder builder;

    auto creds = grpc::sgx::TlsServerCredentials(args.config);
    GPR_ASSERT(creds.get() != nullptr);

    builder.AddListeningPort(args.server_address, creds);

    builder.RegisterService(&service);

    std::unique_ptr<grpc::Server> server(builder.BuildAndStart());
    std::cout << "Server listening on " << args.server_address << std::endl;

    server->Wait();
}

int main(int argc, char** argv) {
    RunServer();
    return 0;
}
