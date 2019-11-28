#include <string>

#include "sofaenclave/ra_report.h"

namespace sofaenclave {
namespace occlum {

#define IMPLEMENT_IAS_REPORT_CLASS_MEMBER(x);                         \
  std::string& IasReport::x() { return x##_; }                        \
  const std::string& IasReport::x() const { return x##_; }            \
  void IasReport::set_##x(const std::string& value) { x##_ = value; } \
  void IasReport::set_##x(const char* value) { x##_ = value; }        \
  void IasReport::set_##x(const char* value, size_t size) { x##_ = value; }

IMPLEMENT_IAS_REPORT_CLASS_MEMBER(b64_signature);
IMPLEMENT_IAS_REPORT_CLASS_MEMBER(signing_cert);
IMPLEMENT_IAS_REPORT_CLASS_MEMBER(advisory_url);
IMPLEMENT_IAS_REPORT_CLASS_MEMBER(advisory_ids);
IMPLEMENT_IAS_REPORT_CLASS_MEMBER(response_body);
IMPLEMENT_IAS_REPORT_CLASS_MEMBER(epid_pseudonym);
IMPLEMENT_IAS_REPORT_CLASS_MEMBER(quote_status);
IMPLEMENT_IAS_REPORT_CLASS_MEMBER(b16_platform_info_blob);
IMPLEMENT_IAS_REPORT_CLASS_MEMBER(b64_quote_body);

}  // namespace occlum
}  // namespace sofaenclave
