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

#include "sgx_ra_tls_backends.h"

namespace grpc {
namespace sgx {

/*
RA-TLS: on client, only need to register ra_tls_verify_callback() for cert verification
  1. extract SGX quote from "quote" OID extension from crt
  2. compare public key's hash from cert against quote's report_data
  3. prepare user-supplied verification parameter "allow outdated TCB"
  4. call into libsgx_dcap_quoteverify to verify ECDSA/based SGX quote
  5. verify all measurements from the SGX quote
*/

std::shared_ptr<grpc::ChannelCredentials> TlsCredentials(sgx_config sgx_cfg) {
    grpc::sgx::CredentialsOptions options;

    ra_tls_parse_sgx_config(sgx_cfg);

    credential_option_set_certificate_provider(options);

    ra_tls_verify_init();
    credential_option_set_authorization_check(options);

    return grpc::experimental::TlsCredentials(
        reinterpret_cast<const grpc::experimental::TlsChannelCredentialsOptions&>(options));
};

std::shared_ptr<grpc::ChannelCredentials> TlsCredentials(const char* sgx_cfg_json) {
    grpc::sgx::CredentialsOptions options;

    ra_tls_parse_sgx_config(sgx_cfg_json);

    credential_option_set_certificate_provider(options);

    ra_tls_verify_init();
    credential_option_set_authorization_check(options);

    return grpc::experimental::TlsCredentials(
        reinterpret_cast<const grpc::experimental::TlsChannelCredentialsOptions&>(options));
};

std::shared_ptr<grpc::ServerCredentials> TlsServerCredentials(sgx_config sgx_cfg) {
    grpc::sgx::CredentialsOptions options;

    ra_tls_parse_sgx_config(sgx_cfg);

    credential_option_set_certificate_provider(options);

    ra_tls_verify_init();
    credential_option_set_authorization_check(options);

    return grpc::experimental::TlsServerCredentials(
        reinterpret_cast<const grpc::experimental::TlsServerCredentialsOptions&>(options));
};

std::shared_ptr<grpc::ServerCredentials> TlsServerCredentials(const char* sgx_cfg_json) {
    grpc::sgx::CredentialsOptions options;

    ra_tls_parse_sgx_config(sgx_cfg_json);

    credential_option_set_certificate_provider(options);

    ra_tls_verify_init();
    credential_option_set_authorization_check(options);

    return grpc::experimental::TlsServerCredentials(
        reinterpret_cast<const grpc::experimental::TlsServerCredentialsOptions&>(options));
};

std::shared_ptr<grpc::Channel> CreateSecureChannel(
    string target_str, std::shared_ptr<grpc::ChannelCredentials> channel_creds) {
    GPR_ASSERT(channel_creds.get() != nullptr);
    auto channel_args = grpc::ChannelArguments();
    channel_args.SetSslTargetNameOverride("RATLS");
    return grpc::CreateCustomChannel(target_str, std::move(channel_creds), channel_args);
};

} // namespace sgx
} // namespace grpc
