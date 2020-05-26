#ifndef REMOTE_ATTESTATION_LIB_INCLUDE_RA_IAS_H_
#define REMOTE_ATTESTATION_LIB_INCLUDE_RA_IAS_H_

#include <mutex>
#include <string>

#include "./sgx_uae_epid.h"
#include "./sgx_urts.h"
#include "./sgx_utils.h"

#include "curl/curl.h"

#include "sofaenclave/common/error.h"
#include "sofaenclave/common/type.h"
#include "sofaenclave/ra_report.h"

namespace sofaenclave {
namespace occlum {

class RaIasClient {
 public:
  /// Connect to the HTTP IAS proxy server
  explicit RaIasClient(const std::string& url);

  /// Connect to the HTTPS IAS
  explicit RaIasClient(const SofaeServerCfg& ias_server);

  ~RaIasClient();

  SofaeErrorCode GetSigRL(const sgx_epid_group_id_t& gid, std::string* sigrl);
  SofaeErrorCode FetchReport(const std::string& quote, IasReport* ias_report);

 private:
  void InitIasConnection(const std::string& url);

  CURL* curl_ = NULL;
  curl_slist* headers_ = NULL;
  std::string server_endpoint_;

  static std::mutex init_mutex_;
};

}  // namespace occlum
}  // namespace sofaenclave

#endif  // REMOTE_ATTESTATION_LIB_INCLUDE_RA_IAS_H_
