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

#ifndef SGX_RA_TLS_BACKENDS_H
#define SGX_RA_TLS_BACKENDS_H

#include "sgx_ra_tls_utils.h"

#include <mutex>
#include <unordered_map>

#include <grpcpp/grpcpp.h>
#include <grpc/grpc_security.h>
#include <grpc/grpc_security_constants.h>
#include <grpcpp/security/credentials.h>
#include <grpcpp/security/tls_certificate_provider.h>
#include <grpcpp/security/tls_credentials_options.h>
#include <grpcpp/security/server_credentials.h>
#include <grpcpp/security/sgx/sgx_ra_tls_options.h>

// Set 1 for strict safety checks
#define SGX_MESUREMENTS_MAX_SIZE 16

namespace grpc {
namespace sgx {

class TlsAuthorizationCheck
    : public grpc::experimental::TlsServerAuthorizationCheckInterface {
    int Schedule(grpc::experimental::TlsServerAuthorizationCheckArg* arg) override;
    void Cancel(grpc::experimental::TlsServerAuthorizationCheckArg* arg) override;
};

struct sgx_measurement {
    char mr_enclave[32];
    char mr_signer[32];
    uint16_t isv_prod_id;
    uint16_t isv_svn;
    bool debuggable;
};

struct sgx_config {
    bool verify_mr_enclave  = true;
    bool verify_mr_signer   = true;
    bool verify_isv_prod_id = true;
    bool verify_isv_svn     = true;
    bool verify_enclave_debuggable = true;
    std::vector<sgx_measurement> sgx_mrs;
};

struct ra_tls_cache {
    int id = 0;
    std::unordered_map<
            int, std::shared_ptr<grpc::experimental::StaticDataCertificateProvider>
        > certificate_provider;
    std::unordered_map<
            int, std::shared_ptr<grpc::sgx::TlsAuthorizationCheck>
        > authorization_check;
    std::unordered_map<
            int, std::shared_ptr<grpc::experimental::TlsServerAuthorizationCheckConfig>
        > authorization_check_config;
};

struct ra_tls_context {
    std::mutex mtx;
    struct sgx_config sgx_cfg;
    struct ra_tls_cache cache;
};

extern struct ra_tls_context _ctx_;


#ifdef SGX_RA_TLS_OCCLUM_BACKEND

std::vector<std::string> occlum_get_key_cert();

int occlum_verify_cert(const unsigned char * der_crt, size_t len);

#endif // SGX_RA_TLS_OCCLUM_BACKEND

void ra_tls_parse_sgx_config(sgx_config sgx_cfg);

void ra_tls_parse_sgx_config(const char* file);

void ra_tls_verify_init();

int verify_measurement(const char* mr_enclave, const char* mr_signer,
                       const char* isv_prod_id, const char* isv_svn,
                       bool debuggable);

void credential_option_set_certificate_provider(grpc::sgx::CredentialsOptions& options);

void credential_option_set_authorization_check(grpc::sgx::CredentialsOptions& options);

} // namespace sgx
} // namespace grpc

#endif // SGX_RA_TLS_BACKENDS_H
