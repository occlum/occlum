#ifndef REMOTE_ATTESTATION_LIB_INCLUDE_SOFAENCLAVE_RA_JSON_H_
#define REMOTE_ATTESTATION_LIB_INCLUDE_SOFAENCLAVE_RA_JSON_H_

#include <map>
#include <memory>
#include <string>
#include <vector>

#include "rapidjson/document.h"

#include "sofaenclave/common/error.h"
#include "sofaenclave/common/log.h"
#include "sofaenclave/common/type.h"

namespace sofaenclave {
namespace occlum {

typedef std::shared_ptr<rapidjson::Document> SofaeJsonDocPtr;
typedef std::map<std::string, SofaeJsonDocPtr> SofaeJsonConfMap;

class JsonConfig {
 public:
  // Gets the singleton UnitTest object.
  static JsonConfig* GetInstance();

  // To support both rapidjson::Document and rapidjson::Value
  template <typename T>
  static bool CheckString(const T& conf, const char* name);
  template <typename T>
  static bool CheckArray(const T& conf, const char* name);
  template <typename T>
  static bool CheckInt(const T& conf, const char* name);
  template <typename T>
  static bool CheckObj(const T& conf, const char* name);
  template <typename T>
  static std::string GetStr(const T& conf, const char* name,
                            const std::string& default_val = "");
  template <typename T>
  static SofaeErrorCode GetStrArray(const T& conf, const char* name,
                                    std::vector<std::string>* values);
  template <typename T>
  static SofaeErrorCode GetInt(const T& conf, const char* name, int* value);

  // Load configuration files and then parse and get value(s)
  std::string ConfGetStr(const std::string& conf_file, const char* name,
                         const std::string& default_val = "");
  SofaeErrorCode ConfGetStrArray(const std::string& conf_file, const char* name,
                                 std::vector<std::string>* values);
  SofaeErrorCode ConfGetInt(const std::string& conf_file, const char* name,
                            int* value);

 private:
  // Hide construction functions
  JsonConfig() {}
  JsonConfig(const JsonConfig&);
  void operator=(JsonConfig const&);

  std::string GetConfigFilename(const std::string& filename);
  SofaeErrorCode LoadConfiguration(const std::string& filename);

  SofaeJsonConfMap cfgs_;
};

}  // namespace occlum
}  // namespace sofaenclave

#endif  // REMOTE_ATTESTATION_LIB_INCLUDE_SOFAENCLAVE_RA_JSON_H_
