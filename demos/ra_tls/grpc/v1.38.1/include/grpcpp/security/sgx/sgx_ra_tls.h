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

#ifndef SGX_RA_TLS_H
#define SGX_RA_TLS_H

#include <memory>

#include <grpcpp/security/credentials.h>
#include <grpcpp/security/server_credentials.h>

#include <cjson/cJSON.h>

namespace grpc {
namespace sgx {

struct sgx_config;

std::vector<std::string> ra_tls_get_key_cert();

void ra_tls_parse_sgx_config(sgx_config sgx_cfg);

void ra_tls_parse_sgx_config(const char* file);

void ra_tls_verify_init();

int ra_tls_auth_check_schedule(void* /* config_user_data */,
                               grpc_tls_server_authorization_check_arg* arg);

std::shared_ptr<grpc::ChannelCredentials> TlsCredentials(sgx_config sgx_cfg);

std::shared_ptr<grpc::ChannelCredentials> TlsCredentials(const char* sgx_cfg_json);

std::shared_ptr<grpc::ServerCredentials> TlsServerCredentials(sgx_config sgx_cfg);

std::shared_ptr<grpc::ServerCredentials> TlsServerCredentials(const char* sgx_cfg_json);

std::shared_ptr<grpc::Channel> CreateSecureChannel(
    string target_str, std::shared_ptr<grpc::ChannelCredentials> channel_creds);

class json_engine {
    public:
        json_engine();

        json_engine(const char*);

        ~json_engine();

        bool open(const char*);

        void close();

        cJSON* get_handle();

        cJSON* get_item(cJSON* obj, const char* item);

        char* print_item(cJSON* obj);

        bool compare_item(cJSON* obj, const char* item);

    private:
        cJSON* handle;
};

}  // namespace sgx
}  // namespace grpc

#endif  // SGX_RA_TLS_H
