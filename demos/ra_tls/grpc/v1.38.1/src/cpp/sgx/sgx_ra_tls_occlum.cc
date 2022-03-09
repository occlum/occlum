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

#ifdef SGX_RA_TLS_OCCLUM_BACKEND

#include <openssl/evp.h>
#include <openssl/rsa.h>
#include <openssl/x509.h>
#include <openssl/x509v3.h>
#include <openssl/sha.h>
#include <openssl/pem.h>
#include <openssl/asn1.h>

#include "sgx_quote_3.h"
#include "occlum_dcap.h"

const char * RA_TLS_LONG_NAME = "RA-TLS Extension";
const char * RA_TLS_SHORT_NAME = "RA-TLS";

std::vector<std::string> occlum_get_key_cert() {
    unsigned char private_key_pem[16000], cert_pem[16000];

    BIGNUM * e = BN_new();
    BN_set_word(e, RSA_F4);
    RSA * rsa = RSA_new();
    RSA_generate_key_ex(rsa, 2048, e, nullptr);

    EVP_PKEY * pkey = EVP_PKEY_new();
    EVP_PKEY_assign_RSA(pkey, rsa);

    X509 * x509 = X509_new();

    ASN1_INTEGER_set(X509_get_serialNumber(x509), 1);
    X509_gmtime_adj(X509_get_notBefore(x509), 0);
    X509_gmtime_adj(X509_get_notAfter(x509), 630720000L);
    X509_set_pubkey(x509, pkey);

    X509_NAME * name = X509_NAME_new();
    // X509_NAME * name = X509_get_subject_name(x509);
    X509_NAME_add_entry_by_txt(name, "C",  MBSTRING_ASC,
                            (unsigned char *)"CN", -1, -1, 0);
    X509_NAME_add_entry_by_txt(name, "O",  MBSTRING_ASC,
                            (unsigned char *)"Intel Inc.", -1, -1, 0);
    X509_NAME_add_entry_by_txt(name, "CN", MBSTRING_ASC,
                            (unsigned char *)"localhost", -1, -1, 0);
    X509_set_subject_name(x509, name);
    X509_set_issuer_name(x509, name);

    int32_t ret;
    size_t key_len = i2d_PUBKEY(pkey, 0);
    unsigned char *public_key = NULL;
    // size_t pubkey_len = i2d_PUBKEY(pkey, &public_key);
    size_t pubkey_len = i2d_X509_PUBKEY(X509_get_X509_PUBKEY(x509), &public_key);

    if (pubkey_len != key_len) {
        grpc_printf("get public key failed!\n");
    }

    BIO *bio = BIO_new(BIO_s_mem());
    if (nullptr == bio) {
        grpc_printf("create bio failed!\n");
    }

    ret = PEM_write_bio_RSAPrivateKey(bio, rsa, nullptr, nullptr, 0, nullptr, nullptr);
    if (ret == 0) {
        grpc_printf("write private key failed!\n");
    }

    ret = BIO_read(bio, private_key_pem, bio->num_write);
    if (ret == 0) {
        grpc_printf("read private key failed!\n");
    }

    unsigned char hash[SHA256_DIGEST_LENGTH];
    SHA256_CTX sha256;
    SHA256_Init(&sha256);
    SHA256_Update(&sha256, public_key, key_len);
    SHA256_Final(hash, &sha256);

    void *handle;
    uint32_t quote_size;
    uint8_t *p_quote_buffer;

    handle = dcap_quote_open();
    quote_size = dcap_get_quote_size(handle);

    p_quote_buffer = (uint8_t*)malloc(quote_size);
    if (nullptr == p_quote_buffer) {
        grpc_printf("Couldn't allocate quote_buffer\n");
    }
    memset(p_quote_buffer, 0, quote_size);

    sgx_report_data_t report_data = { 0 };
    memcpy(report_data.d, hash, SHA256_DIGEST_LENGTH);

    ret = dcap_generate_quote(handle, p_quote_buffer, &report_data);
    if (0 != ret) {
        grpc_printf( "Error in dcap_generate_quote.\n");
    }

    int nid = OBJ_create("1.2.840.113741.1", RA_TLS_SHORT_NAME, RA_TLS_LONG_NAME);
    ASN1_OBJECT* obj = OBJ_nid2obj(nid);  
    ASN1_OCTET_STRING* data = ASN1_OCTET_STRING_new();
    ASN1_OCTET_STRING_set(data, p_quote_buffer, quote_size);

    X509_EXTENSION* ext = X509_EXTENSION_create_by_OBJ(nullptr, obj, 0, data);
    X509_add_ext(x509, ext, -1);

    X509_sign(x509, pkey, EVP_sha1());

    BIO *cert_bio = BIO_new(BIO_s_mem());
    if (nullptr == cert_bio) {
        grpc_printf("create crt bio failed!\n");
    }

    if (0 == PEM_write_bio_X509(cert_bio, x509)) {
        BIO_free(cert_bio);
        grpc_printf("read crt bio failed!\n");
    }

    ret = BIO_read(cert_bio, cert_pem, cert_bio->num_write);
    if (ret == 0) {
        grpc_printf("read pem cert failed!\n");
    }

    BIO_free(bio);
    BIO_free(cert_bio);
    EVP_PKEY_free(pkey);
    check_free(p_quote_buffer);
    dcap_quote_close(handle);

    std::vector<std::string> key_cert;
    key_cert.emplace_back(std::string((char*) private_key_pem));
    key_cert.emplace_back(std::string((char*) cert_pem));
    return key_cert;
}

static int occlum_get_quote(X509 *x509, uint8_t **quote, size_t *len) {
    STACK_OF(X509_EXTENSION) *exts = x509->cert_info->extensions;
    int ext_num;
    int ret = -1; 
    if (exts) {
        ext_num = sk_X509_EXTENSION_num(exts);

        for (int i = 0; i < ext_num; i++) {
        X509_EXTENSION *ext = sk_X509_EXTENSION_value(exts, i);
        ASN1_OBJECT *obj = X509_EXTENSION_get_object(ext);

        unsigned nid = OBJ_obj2nid(obj);
        if (nid != NID_undef) {
            const char *ln = OBJ_nid2ln(nid);
            if (memcmp(RA_TLS_LONG_NAME, ln, sizeof(RA_TLS_LONG_NAME)) == 0) {
            BIO *ext_bio = BIO_new(BIO_s_mem());

            *len = i2d_ASN1_OCTET_STRING(ext->value, quote);
            *quote = *quote + 4;
            *len = *len - 4;
            ret = 0;
            BIO_free(ext_bio);
            }
        }

        }
    }

    return ret;
}

static int occlum_verify_pubkey_hash(X509 *x509, uint8_t *pubkey_hash, size_t len) {
    EVP_PKEY *pkey = X509_get_pubkey(x509);

    int32_t ret;
    size_t key_len = EVP_PKEY_bits(pkey)/8;
    unsigned char *public_key = NULL;

    key_len = i2d_X509_PUBKEY(X509_get_X509_PUBKEY(x509), &public_key);

    unsigned char hash[SHA256_DIGEST_LENGTH];
    SHA256_CTX sha256;
    SHA256_Init(&sha256);
    SHA256_Update(&sha256, public_key, key_len);
    SHA256_Final(hash, &sha256);

    ret = memcmp(hash, pubkey_hash, len);
    return ret;
}

static int occlum_verify_quote(uint8_t * quote_buffer, size_t quote_size) {
    void *handle;
    handle = dcap_quote_open();

    uint32_t supplemental_size, ret;
    uint8_t *p_supplemental_buffer;
    sgx_ql_qv_result_t quote_verification_result = SGX_QL_QV_RESULT_UNSPECIFIED;
    uint32_t collateral_expiration_status = 1;

    supplemental_size = dcap_get_supplemental_data_size(handle);
    p_supplemental_buffer = (uint8_t *)malloc(supplemental_size);
    if (NULL == p_supplemental_buffer) {
        grpc_printf("Couldn't allocate supplemental buffer\n");
    }
    memset(p_supplemental_buffer, 0, supplemental_size);

    ret = dcap_verify_quote(
        handle,
        quote_buffer,
        quote_size,
        &collateral_expiration_status,
        &quote_verification_result,
        supplemental_size,
        p_supplemental_buffer
        );

    if (0 != ret) {
        grpc_printf( "Error in dcap_verify_quote.\n");
    }

    if (collateral_expiration_status != 0) {
        grpc_printf("the verification collateral has expired\n");
    }

    switch (quote_verification_result) {
        case SGX_QL_QV_RESULT_OK:
            grpc_printf("Succeed to verify the quote!\n");
            break;
        case SGX_QL_QV_RESULT_CONFIG_NEEDED:
        case SGX_QL_QV_RESULT_OUT_OF_DATE:
        case SGX_QL_QV_RESULT_OUT_OF_DATE_CONFIG_NEEDED:
        case SGX_QL_QV_RESULT_SW_HARDENING_NEEDED:
        case SGX_QL_QV_RESULT_CONFIG_AND_SW_HARDENING_NEEDED:
            grpc_printf("WARN: App: Verification completed with Non-terminal result: %x\n",
                   quote_verification_result);
            break;
        case SGX_QL_QV_RESULT_INVALID_SIGNATURE:
        case SGX_QL_QV_RESULT_REVOKED:
        case SGX_QL_QV_RESULT_UNSPECIFIED:
        default:
            grpc_printf("\tError: App: Verification completed with Terminal result: %x\n",
                   quote_verification_result);
    }
    check_free(p_supplemental_buffer);
    dcap_quote_close(handle);
    return ret;
}

int occlum_verify_cert(const unsigned char * der_crt, size_t len) {
    BIO* bio = BIO_new(BIO_s_mem());
    BIO_write(bio, der_crt, strlen((const char *)der_crt));
    X509 *x509 = PEM_read_bio_X509(bio, NULL, NULL, NULL);
    
    if (x509 == nullptr) {
        grpc_printf("parse the crt failed! \n");
        return -1;
    }

    uint8_t * quote_buf = nullptr;
    size_t quote_len = 0;
    int ret = occlum_get_quote(x509, &quote_buf, &quote_len);
    if (ret != 0) {
        grpc_printf("parse quote failed!\n");
        return -1;
    }

    ret = occlum_verify_quote(quote_buf, quote_len);
    if (ret != 0) {
        grpc_printf("verify quote failed!\n");
        return -1;
    }

    sgx_quote3_t *p_quote = (sgx_quote3_t *)quote_buf;
    sgx_report_body_t *p_rep_body = (sgx_report_body_t *)(&p_quote->report_body);
    sgx_report_data_t *p_rep_data =(sgx_report_data_t *)(&p_rep_body->report_data);
    uint8_t *pubkey_hash = p_rep_data->d;


    ret = occlum_verify_pubkey_hash(x509, pubkey_hash, SHA256_DIGEST_LENGTH);
    if (ret != 0) {
        grpc_printf("verify the public key hash failed!\n");
        return -1;
    }

    // Check if enclave is debuggable
    bool debuggable = false;
    if (p_rep_body->attributes.flags & SGX_FLAGS_DEBUG)
        debuggable = true;

    ret = verify_measurement((const char *)&p_rep_body->mr_enclave,
                             (const char *)&p_rep_body->mr_signer,
                             (const char *)&p_rep_body->isv_prod_id,
                             (const char *)&p_rep_body->isv_svn,
                             debuggable);
    if (ret != 0) {
        grpc_printf("verify the measurement failed!\n");
        return -1;
    }

    BIO_free(bio);
    return 0;
}

#endif // SGX_RA_TLS_OCCLUM_BACKEND

} // namespace sgx
} // namespace grpc
