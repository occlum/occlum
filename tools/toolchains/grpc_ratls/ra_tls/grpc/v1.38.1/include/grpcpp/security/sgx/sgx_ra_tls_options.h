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

#ifndef SGX_RA_TLS_OPTIONS_H
#define SGX_RA_TLS_OPTIONS_H

#include <grpc/grpc_security_constants.h>
#include <grpc/status.h>
#include <grpc/support/log.h>
#include <grpcpp/security/tls_certificate_provider.h>
#include <grpcpp/security/tls_credentials_options.h>
#include <grpcpp/support/config.h>

#include <memory>
#include <vector>

namespace grpc {
namespace sgx {

// Contains configurable options on the client side.
// Client side doesn't need to always use certificate provider. When the
// certificate provider is not set, we will use the root certificates stored
// in the system default locations, and assume client won't provide any
// identity certificates(single side TLS).
// It is used for experimental purposes for now and it is subject to change.
class CredentialsOptions final : public grpc::experimental::TlsCredentialsOptions {
 public:

  explicit CredentialsOptions() : TlsCredentialsOptions() {}

  // Sets option to request the certificates from the client.
  // The default is GRPC_SSL_DONT_REQUEST_CLIENT_CERTIFICATE.
  void set_cert_request_type(
      grpc_ssl_client_certificate_request_type cert_request_type);

  // Sets the option to verify the server.
  // The default is GRPC_TLS_SERVER_VERIFICATION.
  void set_verification_option(
      grpc_tls_server_verification_option server_verification_option);

  // Sets the custom authorization config.
  void set_authorization_check_config(
      std::shared_ptr<grpc::experimental::TlsServerAuthorizationCheckConfig>
          authorization_check_config);

 private:
};

}  // namespace sgx
}  // namespace grpc

#endif  // SGX_RA_TLS_OPTIONS_H
