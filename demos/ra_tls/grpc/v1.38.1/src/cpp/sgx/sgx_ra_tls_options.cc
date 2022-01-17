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

#include <grpc/grpc_security.h>
#include <grpc/support/alloc.h>
#include <grpcpp/security/sgx/sgx_ra_tls_options.h>

#include "absl/container/inlined_vector.h"
#include "src/cpp/common/tls_credentials_options_util.h"

namespace grpc {
namespace sgx {

void CredentialsOptions::set_verification_option(
    grpc_tls_server_verification_option server_verification_option) {
    grpc_tls_credentials_options* options = c_credentials_options();
    GPR_ASSERT(options != nullptr);
    grpc_tls_credentials_options_set_server_verification_option(
        options, server_verification_option);
}

void CredentialsOptions::set_authorization_check_config(
    std::shared_ptr<grpc::experimental::TlsServerAuthorizationCheckConfig> config) {
    grpc_tls_credentials_options* options = c_credentials_options();
    GPR_ASSERT(options != nullptr);
    if (config != nullptr) {
        grpc_tls_credentials_options_set_server_authorization_check_config(
            options, config->c_config());
    }
}

void CredentialsOptions::set_cert_request_type(
    grpc_ssl_client_certificate_request_type cert_request_type) {
    grpc_tls_credentials_options* options = c_credentials_options();
    GPR_ASSERT(options != nullptr);
    grpc_tls_credentials_options_set_cert_request_type(options, cert_request_type);
}

} // namespace sgx
} // namespace grpc
