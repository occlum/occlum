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

struct ra_tls_context _ctx_;

std::vector<std::string> ra_tls_get_key_cert() {
#ifdef SGX_RA_TLS_OCCLUM_BACKEND
    return occlum_get_key_cert();
#endif
}

static std::vector<grpc::experimental::IdentityKeyCertPair> get_identity_key_cert_pairs(
    std::vector<std::string> key_cert) {
    grpc::experimental::IdentityKeyCertPair key_cert_pair;
    key_cert_pair.private_key = key_cert[0];
    key_cert_pair.certificate_chain = key_cert[1];
    std::vector<grpc::experimental::IdentityKeyCertPair> identity_key_cert_pairs;
    identity_key_cert_pairs.emplace_back(key_cert_pair);
    return identity_key_cert_pairs;
}

void credential_option_set_certificate_provider(grpc::sgx::CredentialsOptions& options) {
    std::lock_guard<std::mutex> lock(_ctx_.mtx);

    _ctx_.cache.id++;

    auto certificate_provider = _ctx_.cache.certificate_provider.insert({
            _ctx_.cache.id,
            std::make_shared<grpc::experimental::StaticDataCertificateProvider>(
                get_identity_key_cert_pairs(ra_tls_get_key_cert()))
        }).first;

    options.set_certificate_provider(certificate_provider->second);
    options.watch_identity_key_cert_pairs();
    options.set_cert_request_type(GRPC_SSL_REQUEST_AND_REQUIRE_CLIENT_CERTIFICATE_BUT_DONT_VERIFY);
    options.set_root_cert_name("");
    options.set_identity_cert_name("");
}

static sgx_config parse_sgx_config_json(const char* file) {
    class json_engine sgx_json(file);
    struct sgx_config sgx_cfg;

    sgx_cfg.verify_mr_enclave = sgx_json.compare_item(sgx_json.get_item(sgx_json.get_handle(), "verify_mr_enclave"), "on");
    sgx_cfg.verify_mr_signer = sgx_json.compare_item(sgx_json.get_item(sgx_json.get_handle(), "verify_mr_signer"), "on");
    sgx_cfg.verify_isv_prod_id = sgx_json.compare_item(sgx_json.get_item(sgx_json.get_handle(), "verify_isv_prod_id"), "on");
    sgx_cfg.verify_isv_svn = sgx_json.compare_item(sgx_json.get_item(sgx_json.get_handle(), "verify_isv_svn"), "on");
    sgx_cfg.verify_enclave_debuggable =
        sgx_json.compare_item(sgx_json.get_item(sgx_json.get_handle(), "verify_enclave_debuggable"), "on");

    auto objs = sgx_json.get_item(sgx_json.get_handle(), "sgx_mrs");
    auto obj_num = std::min(cJSON_GetArraySize(objs), SGX_MESUREMENTS_MAX_SIZE);

    sgx_cfg.sgx_mrs = std::vector<sgx_measurement>(obj_num, sgx_measurement());
    for (auto i = 0; i < obj_num; i++) {
        auto obj = cJSON_GetArrayItem(objs, i);

        auto mr_enclave = sgx_json.print_item(sgx_json.get_item(obj, "mr_enclave"));
        memset(sgx_cfg.sgx_mrs[i].mr_enclave, 0, sizeof(sgx_cfg.sgx_mrs[i].mr_enclave));
        hex_to_byte(mr_enclave+1, sgx_cfg.sgx_mrs[i].mr_enclave, sizeof(sgx_cfg.sgx_mrs[i].mr_enclave));

        auto mr_signer = sgx_json.print_item(sgx_json.get_item(obj, "mr_signer"));
        memset(sgx_cfg.sgx_mrs[i].mr_signer, 0, sizeof(sgx_cfg.sgx_mrs[i].mr_signer));
        hex_to_byte(mr_signer+1, sgx_cfg.sgx_mrs[i].mr_signer, sizeof(sgx_cfg.sgx_mrs[i].mr_signer));

        auto isv_prod_id = sgx_json.print_item(sgx_json.get_item(obj, "isv_prod_id"));
        sgx_cfg.sgx_mrs[i].isv_prod_id = strtoul(isv_prod_id, nullptr, 10);

        auto isv_svn = sgx_json.print_item(sgx_json.get_item(obj, "isv_svn"));
        sgx_cfg.sgx_mrs[i].isv_svn = strtoul(isv_svn, nullptr, 10);

        if (cJSON_IsTrue(sgx_json.get_item(obj, "debuggable")) == 0)
            sgx_cfg.sgx_mrs[i].debuggable = false;
        else
            sgx_cfg.sgx_mrs[i].debuggable = true;
    };
    return sgx_cfg;
}

void ra_tls_parse_sgx_config(sgx_config sgx_cfg) {
    std::lock_guard<std::mutex> lock(_ctx_.mtx);
    _ctx_.sgx_cfg = sgx_cfg;
}

void ra_tls_parse_sgx_config(const char* file) {
    ra_tls_parse_sgx_config(parse_sgx_config_json(file));
}

void ra_tls_verify_init() {
    std::lock_guard<std::mutex> lock(_ctx_.mtx);
}

static bool verify_measurement_internal(const char* mr_enclave, const char* mr_signer,
                                        const char* isv_prod_id, const char* isv_svn,
                                        bool debuggable) {
    bool status = false;
    auto & sgx_cfg = _ctx_.sgx_cfg;
    for (auto & obj : sgx_cfg.sgx_mrs) {
        status = true;

        if (status && sgx_cfg.verify_mr_enclave && \
            memcmp(obj.mr_enclave, mr_enclave, 32)) {
            status = false;
        }

        if (status && sgx_cfg.verify_mr_signer && \
            memcmp(obj.mr_signer, mr_signer, 32)) {
            status = false;
        }

        if (status && sgx_cfg.verify_isv_prod_id && \
            (obj.isv_prod_id != *(uint16_t*)isv_prod_id)) {
            status = false;
        }

        if (status && sgx_cfg.verify_isv_svn && \
            (obj.isv_svn != *(uint16_t*)isv_svn)) {
            status = false;
        }

        if (status && sgx_cfg.verify_enclave_debuggable && \
            (obj.debuggable != debuggable)) {
            status = false;
        }

        if (status) {
            break;
        }
    }
    return status;
}

int verify_measurement(const char* mr_enclave, const char* mr_signer,
                       const char* isv_prod_id, const char* isv_svn,
                       bool debuggable) {
    std::lock_guard<std::mutex> lock(_ctx_.mtx);
    bool status = false;
    try {
        assert(mr_enclave && mr_signer && isv_prod_id && isv_svn);
        status = verify_measurement_internal(mr_enclave, mr_signer, isv_prod_id, isv_svn, debuggable);
        grpc_printf("remote sgx measurements\n"); 
        grpc_printf("  |- mr_enclave     :  %s\n", byte_to_hex(mr_enclave, 32).c_str());
        grpc_printf("  |- mr_signer      :  %s\n", byte_to_hex(mr_signer, 32).c_str());
        grpc_printf("  |- isv_prod_id    :  %hu\n", *((uint16_t*)isv_prod_id));
        grpc_printf("  |- isv_svn        :  %hu\n", *((uint16_t*)isv_svn));
        grpc_printf("  |- debuggable     :  %s", debuggable?"true":"false");
        if (status) {
            grpc_printf("  |- verify result  :  success\n");
        } else {
            grpc_printf("  |- verify result  :  failed\n");
        }
    } catch (...) {
        grpc_printf("unable to verify measurement!");
    }

    fflush(stdout);
    return status ? 0 : -1;
}

int TlsAuthorizationCheck::Schedule(grpc::experimental::TlsServerAuthorizationCheckArg* arg) {
    GPR_ASSERT(arg != nullptr);

    char der_crt[16000] = "";
    auto peer_cert_buf = arg->peer_cert();
    peer_cert_buf.copy(der_crt, peer_cert_buf.length(), 0);

#ifdef SGX_RA_TLS_OCCLUM_BACKEND
    int ret = occlum_verify_cert((const unsigned char *)der_crt, 16000);
#endif

    if (ret != 0) {
        grpc_printf("something went wrong while verifying quote\n");
        arg->set_success(0);
        arg->set_status(GRPC_STATUS_UNAUTHENTICATED);
    } else {
        arg->set_success(1);
        arg->set_status(GRPC_STATUS_OK);
    }
    return 0;
};

void TlsAuthorizationCheck::Cancel(grpc::experimental::TlsServerAuthorizationCheckArg* arg) {
    GPR_ASSERT(arg != nullptr);
    arg->set_status(GRPC_STATUS_PERMISSION_DENIED);
    arg->set_error_details("cancelled");
};

int ra_tls_auth_check_schedule(void* /* confiuser_data */,
                               grpc_tls_server_authorization_check_arg* arg) {
    char der_crt[16000] = "";
    memcpy(der_crt, arg->peer_cert, strlen(arg->peer_cert));

#ifdef SGX_RA_TLS_OCCLUM_BACKEND
    int ret = occlum_verify_cert((const unsigned char *)der_crt, 16000);
#endif

    if (ret != 0) {
        grpc_printf("something went wrong while verifying quote\n");
        arg->success = 0;
        arg->status = GRPC_STATUS_UNAUTHENTICATED;
    } else {
        arg->success = 1;
        arg->status = GRPC_STATUS_OK;
    }
    return 0;
}

void credential_option_set_authorization_check(grpc::sgx::CredentialsOptions& options) {
    std::lock_guard<std::mutex> lock(_ctx_.mtx);

    _ctx_.cache.id++;

    auto authorization_check = _ctx_.cache.authorization_check.insert({
            _ctx_.cache.id, std::make_shared<grpc::sgx::TlsAuthorizationCheck>()
        }).first;

    auto authorization_check_config = _ctx_.cache.authorization_check_config.insert({
            _ctx_.cache.id,
            std::make_shared<grpc::experimental::TlsServerAuthorizationCheckConfig>(
                authorization_check->second)
        }).first;

    options.set_authorization_check_config(authorization_check_config->second);
    options.set_verification_option(GRPC_TLS_SKIP_ALL_SERVER_VERIFICATION);
}

} // namespace sgx
} // namespace grpc
