#include <fstream>
#include <iostream>
#include <string>
#include <vector>

#include "sofaenclave/common/error.h"
#include "sofaenclave/common/log.h"
#include "sofaenclave/ra_json.h"

namespace sofaenclave {
namespace occlum {

static SofaeErrorCode FsReadString(const std::string& filename,
                                   std::string* str) {
  std::ifstream ifs(filename, std::ios::binary | std::ios::in);
  if (!ifs) {
    SOFAE_LOG_ERROR("Fail to open file \"%s\"\n", filename.c_str());
    return SOFAE_ERROR_FILE_OPEN;
  }

  ifs.seekg(0, std::ios::end);
  int length = ifs.tellg();
  ifs.seekg(0, std::ios::beg);

  std::vector<char> buf(length);
  ifs.read(buf.data(), length);
  if (ifs.fail()) {
    SOFAE_LOG_ERROR("Fail to read file \"%s\"\n", filename.c_str());
    return SOFAE_ERROR_FILE_READ;
  }

  str->assign(buf.data(), length);
  return SOFAE_SUCCESS;
}

static bool FsFileExists(const std::string& filename) {
  std::ifstream ifs(filename, std::ios::binary | std::ios::in);
  return ifs.good();
}

JsonConfig* JsonConfig::GetInstance() {
  static JsonConfig instance;
  return &instance;
}

template <typename T>
bool JsonConfig::CheckString(const T& conf, const char* name) {
  if (!conf.HasMember(name) || !conf[name].IsString()) {
    SOFAE_LOG_ERROR("%s is missed or not string in config file", name);
    return false;
  }
  return true;
}

template <typename T>
bool JsonConfig::CheckArray(const T& conf, const char* name) {
  if (!conf.HasMember(name) || !conf[name].IsArray()) {
    SOFAE_LOG_ERROR("%s is missed or not array in config file", name);
    return false;
  }
  return true;
}

template <typename T>
bool JsonConfig::CheckInt(const T& conf, const char* name) {
  if (!conf.HasMember(name) || !conf[name].IsInt()) {
    SOFAE_LOG_ERROR("%s is missed or not integer in config file", name);
    return false;
  }
  return true;
}

template <typename T>
bool JsonConfig::CheckObj(const T& conf, const char* name) {
  if (!conf.HasMember(name) || !conf[name].IsObject()) {
    SOFAE_LOG_ERROR("%s is missed or not object in config file", name);
    return false;
  }
  return true;
}

template <typename T>
std::string JsonConfig::GetStr(const T& conf, const char* name,
                               const std::string& default_val) {
  if (CheckString(conf, name)) {
    std::string value = conf[name].GetString();
    SOFAE_LOG_DEBUG("%s=%s", name, value.c_str());
    return value;
  } else {
    SOFAE_LOG_DEBUG("Not string type, %s=%s[default]", name, default_val);
    return default_val;
  }
}

template <typename T>
SofaeErrorCode JsonConfig::GetStrArray(const T& conf, const char* name,
                                       std::vector<std::string>* values) {
  if (CheckArray(conf, name)) {
    const rapidjson::Value& val_array = conf[name];
    size_t count = val_array.Size();
    for (size_t i = 0; i < count; i++) {
      if (val_array[i].IsString()) {
        std::string val_str = val_array[i].GetString();
        SOFAE_LOG_DEBUG("%s[%ld]=%s", name, i, val_str.c_str());
        values->push_back(val_str);
      } else {
        SOFAE_LOG_ERROR("Invalid string type in Array");
        return SOFAE_ERROR_PARSE_CONFIGURATIONS;
      }
    }
  } else {
    SOFAE_LOG_DEBUG("Invalid Array type");
    return SOFAE_ERROR_PARSE_CONFIGURATIONS;
  }
  return SOFAE_SUCCESS;
}

template <typename T>
SofaeErrorCode JsonConfig::GetInt(const T& conf, const char* name, int* value) {
  if (!CheckInt(conf, name)) {
    SOFAE_LOG_ERROR("Not integer type: %s", name);
    return SOFAE_ERROR_PARSE_CONFIGURATIONS;
  }

  *value = conf[name].GetInt();
  SOFAE_LOG_DEBUG("%s=%d", name, *value);
  return SOFAE_SUCCESS;
}

std::string JsonConfig::GetConfigFilename(const std::string& filename) {
  // First priority, the absolute path filename or file in current directory
  if (FsFileExists(filename)) {
    SOFAE_LOG_DEBUG("Configuration file: %s", filename.c_str());
    return filename;
  }

  // Finally, try to find configuration file in /etc directory
  std::string etcpath = "/etc/";
  etcpath += filename;
  if (FsFileExists(etcpath)) {
    SOFAE_LOG_DEBUG("Configuration file: %s", etcpath.c_str());
    return etcpath;
  }

  // If cannot find configuration file, return empty string
  SOFAE_LOG_ERROR("Cannot find configuration file: %s", filename.c_str());
  return "";
}

SofaeErrorCode JsonConfig::LoadConfiguration(const std::string& filename) {
  if (filename.empty()) {
    SOFAE_LOG_ERROR("Empty configuration file name");
    return SOFAE_ERROR_CONF_NOTEXIST;
  }

  std::string config_file = GetConfigFilename(filename);
  if (config_file.empty()) {
    SOFAE_LOG_ERROR("Fail to find configuration file");
    return SOFAE_ERROR_CONF_NOTEXIST;
  }

  std::string config_str;
  if (FsReadString(config_file, &config_str) != SOFAE_SUCCESS) {
    SOFAE_LOG_ERROR("Fail to read configuration file");
    return SOFAE_ERROR_PARSE_CONFIGURATIONS;
  }

  SofaeJsonDocPtr doc(new rapidjson::Document);
  if (doc.get()->Parse(config_str.data()).HasParseError()) {
    SOFAE_LOG_ERROR("Fail to parse json configration file");
    return SOFAE_ERROR_PARSE_CONFIGURATIONS;
  }

  cfgs_.emplace(filename, doc);
  SOFAE_LOG_DEBUG("Load configuration file %s successfully", filename.c_str());
  return SOFAE_SUCCESS;
}

std::string JsonConfig::ConfGetStr(const std::string& conf_file,
                                   const char* name,
                                   const std::string& default_val) {
  SOFAE_LOG_DEBUG("Get %s from %s", name, conf_file.c_str());

  if (cfgs_.find(conf_file) == cfgs_.end()) {
    if (LoadConfiguration(conf_file) != SOFAE_SUCCESS) {
      SOFAE_LOG_DEBUG("Load config failed, %s=%s[default]", name, default_val);
      return default_val;
    }
  }

  return GetStr(*cfgs_[conf_file].get(), name, default_val);
}

SofaeErrorCode JsonConfig::ConfGetStrArray(const std::string& conf_file,
                                           const char* name,
                                           std::vector<std::string>* values) {
  SOFAE_LOG_DEBUG("Get %s from %s", name, conf_file.c_str());

  if (cfgs_.find(conf_file) == cfgs_.end()) {
    if (LoadConfiguration(conf_file) != SOFAE_SUCCESS) {
      SOFAE_LOG_DEBUG("Fail to load configuration file");
      return SOFAE_ERROR_PARSE_CONFIGURATIONS;
    }
  }

  return GetStrArray(*cfgs_[conf_file].get(), name, values);
}

SofaeErrorCode JsonConfig::ConfGetInt(const std::string& conf_file,
                                      const char* name, int* value) {
  SOFAE_LOG_DEBUG("Get %s from %s", name, conf_file.c_str());

  if (cfgs_.find(conf_file) == cfgs_.end()) {
    if (LoadConfiguration(conf_file) != SOFAE_SUCCESS) {
      SOFAE_LOG_ERROR("Fail to load configuration file");
      return SOFAE_ERROR_PARSE_CONFIGURATIONS;
    }
  }

  return GetInt(*cfgs_[conf_file].get(), name, value);
}

}  // namespace occlum
}  // namespace sofaenclave

#ifdef __cplusplus
extern "C" {
#endif

std::string SofaeConfGetStr(const std::string& conf_file, const char* name,
                            const std::string& default_val) {
  return sofaenclave::occlum::JsonConfig::GetInstance()->ConfGetStr(
      conf_file, name, default_val);
}

SofaeErrorCode SofaeConfGetStrArray(const std::string& conf_file,
                                    const char* name,
                                    std::vector<std::string>* values) {
  return sofaenclave::occlum::JsonConfig::GetInstance()->ConfGetStrArray(
      conf_file, name, values);
}

SofaeErrorCode SofaeConfGetInt(const std::string& conf_file, const char* name,
                               int* value) {
  return sofaenclave::occlum::JsonConfig::GetInstance()->ConfGetInt(
      conf_file, name, value);
}

#ifdef __cplusplus
}
#endif
