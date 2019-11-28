#ifndef REMOTE_ATTESTATION_LIB_INCLUDE_SOFAENCLAVE_RA_CONF_H_
#define REMOTE_ATTESTATION_LIB_INCLUDE_SOFAENCLAVE_RA_CONF_H_

#include <string>
#include <vector>

#ifdef __cplusplus
extern "C" {
#endif

std::string SofaeConfGetStr(const std::string& conf_file, const char* name,
                            const std::string& default_value = "");
SofaeErrorCode SofaeConfGetStrArray(const std::string& conf_file,
                                    const char* name,
                                    std::vector<std::string>* values);
SofaeErrorCode SofaeConfGetInt(const std::string& conf_file, const char* name,
                               int* value);

#ifdef __cplusplus
}
#endif

#endif  // REMOTE_ATTESTATION_LIB_INCLUDE_SOFAENCLAVE_RA_CONF_H_
