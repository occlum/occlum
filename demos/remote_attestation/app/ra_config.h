#ifndef REMOTE_ATTESTATION_RA_CONFIG_H_
#define REMOTE_ATTESTATION_RA_CONFIG_H_

#include <string>

#include "sofaenclave/common/error.h"
#include "sofaenclave/common/log.h"
#include "sofaenclave/ra_conf.h"

constexpr char kConfValueEnable[] = "enable";
constexpr char kConfValueDisable[] = "disable";
constexpr char kConfValueTrue[] = "true";
constexpr char kConfValueFalse[] = "false";

constexpr char kRaConf[] = "ra_config.json";

constexpr char kConfIasServer[] = "ias_url";
constexpr char kConfIasCert[] = "ias_sp_cert_file";
constexpr char kConfIasKey[] = "ias_sp_key_file";
constexpr char kConfSPID[] = "enclave_spid";

#define RA_CONF_STR(name) SofaeConfGetStr(kRaConf, name)
#define RA_CONF_INT(name, value) SofaeConfGetInt(kRaConf, name, value)
#define RA_CONF_ARRARY(name, value) SofaeConfGetStrArray(kRaConf, name, value)

#endif  // REMOTE_ATTESTATION_RA_CONFIG_H_
