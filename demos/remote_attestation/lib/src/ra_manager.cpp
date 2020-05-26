#include <algorithm>
#include <cstring>
#include <string>
#include <vector>

#include "./sgx_quote.h"
#include "./sgx_tseal.h"

#include "sofaenclave/common/error.h"
#include "sofaenclave/common/log.h"
#include "sofaenclave/common/type.h"
#include "sofaenclave/ra_device.h"
#include "sofaenclave/ra_ias.h"
#include "sofaenclave/ra_manager.h"

// use cppcodec/base64
#include "cppcodec/base64_rfc4648.hpp"
using base64 = cppcodec::base64_rfc4648;

#ifdef __cplusplus
extern "C" {
#endif

SofaeErrorCode InitializeQuote(sgx_epid_group_id_t* gid) {
  return sofaenclave::occlum::SgxDeviceGetGroupID(gid);
}

SofaeErrorCode GetQuote(SofaeQuoteArgs* quote_args) {
  if (!quote_args->quote.as_buf || (quote_args->quote_buf_len == 0)) {
    SOFAE_LOG_ERROR("Invalid quote buffer or len");
    return SOFAE_ERROR_PARAMETERS;
  }

  return sofaenclave::occlum::SgxDeviceGetQuote(quote_args);
}

SofaeErrorCode FetchIasSigRL(const SofaeServerCfg& ias_server,
                             const sgx_epid_group_id_t& gid,
                             std::string* sigrl) {
  sofaenclave::occlum::RaIasClient ias_client(ias_server);
  return ias_client.GetSigRL(gid, sigrl);
}

SofaeErrorCode FetchIasReport(const SofaeServerCfg& ias_server,
                              sgx_quote_t* quote,
                              SofaeIasReport* ias_report) {
  sofaenclave::occlum::RaIasClient ias_client(ias_server);
  std::string quote_str(RCAST(char*, quote),
                        sizeof(sgx_quote_t) + quote->signature_len);
  return ias_client.FetchReport(quote_str, ias_report);
}

SofaeErrorCode GetQuoteAndFetchIasReport(const SofaeServerCfg& ias_server,
                                         SofaeQuoteArgs* quote_args,
                                         SofaeIasReport* ias_report) {
  // Initialize the quote firstly
  sgx_epid_group_id_t gid = {0};
  SofaeErrorCode ret = InitializeQuote(&gid);
  if (ret != SOFAE_SUCCESS) {
    return ret;
  }

  // If there is no SigRL, try to fetch it.
  if (!quote_args->sigrl_ptr || (quote_args->sigrl_len == 0)) {
    std::string sigrl_str;
    ret = FetchIasSigRL(ias_server, gid, &sigrl_str);
    if (ret != SOFAE_SUCCESS) {
      return ret;
    }
    if (!sigrl_str.empty()) {
      quote_args->sigrl_ptr = RCAST(const uint8_t *, sigrl_str.data());
      quote_args->sigrl_len = sigrl_str.length();
    }
  }

  // Get the quote, assuming quote buffer is allocated out of this function
  ret = GetQuote(quote_args);
  if (ret != SOFAE_SUCCESS) {
    return ret;
  }

  // Fetch the IAS report based on the quote output buffer
  return FetchIasReport(ias_server, quote_args->quote.as_quote, ias_report);
}

#ifdef __cplusplus
}
#endif
