#ifndef REMOTE_ATTESTATION_LIB_INCLUDE_SOFAENCLAVE_COMMON_TYPE_H_
#define REMOTE_ATTESTATION_LIB_INCLUDE_SOFAENCLAVE_COMMON_TYPE_H_

#include <string>

#include "./sgx_quote.h"
#include "./sgx_report.h"
#include "./sgx_tseal.h"

#define RCAST(t, v) reinterpret_cast<t>((v))
#define SCAST(t, v) static_cast<t>((v))
#define CCAST(t, v) const_cast<t>((v))

#define SOFAE_UNREFERENCED_PARAMETER(p) static_cast<void>((p))

typedef uint8_t SofaeEnclaveQuote[4096];

/**
 * report_data    Input report data which will be included in quote data.
 *                The first 32 bytes should be the SHA256 hash value of
 *                the public key which is used in the RA work flow.
 * nonce          Nonce value to avoid replay attack. All zero to ignore it.
 * spid           The service provider ID, please use you real SPID,
 *                otherwise, IAS will return bad request when quote report.
 * quote_type     Maybe SGX_UNLINKABLE_SIGNATURE or SGX_LINKABLE_SIGNATURE
 *                quote type.
 * sigrl_ptr      The SigRL data buffer
 * sigrl_len      The total length of SigRL data
 * quote          Output quote structure data in binary format.
 */
typedef struct {
  sgx_report_data_t report_data;     // input
  sgx_quote_sign_type_t quote_type;  // input
  sgx_spid_t spid;                   // input
  sgx_quote_nonce_t nonce;           // input
  const uint8_t* sigrl_ptr;          // input (optional)
  uint32_t sigrl_len;                // input (optional)
  uint32_t quote_buf_len;            // input
  union {
    uint8_t* as_buf;
    sgx_quote_t* as_quote;
  } quote;  // output
} SofaeQuoteArgs;

/**
 * endpoint       http://xxx.xxx.xxx.xxx:<port> for HTTP IAS proxy server
 *                or https://xxx.xxx.xxx.xxx:<port> for IAS server. Key and
 *                certificate must be provoided for HTTPS IAS server.
 * cert           Service provider certificate file path
 * key            Service provider private key file path
 */
typedef struct {
  std::string endpoint;
  std::string cert;
  std::string key;
} SofaeServerCfg;

#endif  // REMOTE_ATTESTATION_LIB_INCLUDE_SOFAENCLAVE_COMMON_TYPE_H_
