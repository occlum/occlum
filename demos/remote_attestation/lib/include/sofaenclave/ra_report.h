#ifndef REMOTE_ATTESTATION_LIB_INCLUDE_RA_REPORT_H_
#define REMOTE_ATTESTATION_LIB_INCLUDE_RA_REPORT_H_

#include <string>

namespace sofaenclave {
namespace occlum {

#define DECLARE_IAS_REPORT_CLASS_MEMBER(x)      \
 public:                                        \
  std::string& x();                             \
  const std::string& x() const;                 \
  void set_##x(const std::string& value);       \
  void set_##x(const char* value);              \
  void set_##x(const char* value, size_t size); \
                                                \
 private:                                       \
  std::string x##_

class IasReport {
 public:
  IasReport() {}
  ~IasReport() {}

  DECLARE_IAS_REPORT_CLASS_MEMBER(b64_signature);
  DECLARE_IAS_REPORT_CLASS_MEMBER(signing_cert);
  DECLARE_IAS_REPORT_CLASS_MEMBER(advisory_url);
  DECLARE_IAS_REPORT_CLASS_MEMBER(advisory_ids);
  DECLARE_IAS_REPORT_CLASS_MEMBER(response_body);
  DECLARE_IAS_REPORT_CLASS_MEMBER(epid_pseudonym);
  DECLARE_IAS_REPORT_CLASS_MEMBER(quote_status);
  DECLARE_IAS_REPORT_CLASS_MEMBER(b16_platform_info_blob);
  DECLARE_IAS_REPORT_CLASS_MEMBER(b64_quote_body);
};

}  // namespace occlum
}  // namespace sofaenclave

// For coding convenience
using SofaeIasReport = sofaenclave::occlum::IasReport;

#endif  // REMOTE_ATTESTATION_LIB_INCLUDE_RA_REPORT_H_
