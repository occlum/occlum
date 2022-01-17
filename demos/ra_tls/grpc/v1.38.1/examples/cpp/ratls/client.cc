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

class GreeterClient {
    public:
        GreeterClient(std::shared_ptr<grpc::Channel> channel) : stub_(Greeter::NewStub(channel)) {}

        std::string SayHello(const std::string& user) {
            HelloRequest request;
            request.set_name(user);

            HelloReply reply;

            grpc::ClientContext context;

            grpc::Status status = stub_->SayHello(&context, request, &reply);

            if (status.ok()) {
                return reply.message();
            } else {
                std::cout << status.error_code() << ": " << status.error_message() << std::endl;
                return "RPC failed";
            }
        }

    private:
        std::unique_ptr<Greeter::Stub> stub_;
};

void run_client() {
    argparser args;

    auto cred = grpc::sgx::TlsCredentials(args.config);
    auto channel = grpc::CreateChannel(args.server_address, cred);

    GreeterClient greeter(channel);

    std::string user_a = greeter.SayHello("a");
    std::string user_b = greeter.SayHello("b");

    std::cout << "Greeter received: " << user_a << ", "<< user_b << std::endl;
};

int main(int argc, char** argv) {
    run_client();
    return 0;
}
