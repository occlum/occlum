#include <cstring>
#include <string>

#include "./ra_config.h"
#include "sofaenclave/ra_manager.h"

static uint8_t Hex2Dec(const char hex) {
  if (('0' <= hex) && (hex <= '9')) {
    return hex - '0';
  }
  else if (('a' <= hex) && (hex <= 'f')) {
    return hex - 'a' + 10;
  }
  else if (('A' <= hex) && (hex <= 'F')) {
    return hex - 'A' + 10;
  }
  else {
    // Otherwise return zero for none HEX charactor
    return 0;
  }
}

static std::vector<uint8_t> HexStr2Bytes(const uint8_t* str) {
  int len = strlen(RCAST(const char *, str)) / 2;
  std::vector<uint8_t> dst(len);

  for (int i = 0; i < len; i++) {
    dst[i] = (Hex2Dec(str[i * 2] & 0xFF) << 4) +
             (Hex2Dec(str[i * 2 + 1] & 0xFF));
  }
  return dst;
}

int main() {
  printf("Remote attestation testing ...\n");

  std::string endpoint = RA_CONF_STR(kConfIasServer);
  std::string cert = RA_CONF_STR(kConfIasCert);
  std::string key = RA_CONF_STR(kConfIasKey);
  std::string access_key = RA_CONF_STR(kConfIasAccessKey);
  std::string spid_str = RA_CONF_STR(kConfSPID);

  if (spid_str.empty()) {
    printf("Please specify the right SPID in configuration file!\n");
    return -1;
  }

  SofaeServerCfg ias_server = {endpoint, cert, key, access_key};
  SofaeEnclaveQuote quote = {0};
  SofaeQuoteArgs quote_args = {0};
  quote_args.quote_type = SGX_LINKABLE_SIGNATURE;
  quote_args.quote.as_buf = RCAST(uint8_t *, &quote);
  quote_args.quote_buf_len = sizeof(SofaeEnclaveQuote);

  std::vector<uint8_t> spid_vec = HexStr2Bytes(
      RCAST(const uint8_t *, spid_str.c_str()));
  std::memcpy(RCAST(void *, &quote_args.spid.id),
      RCAST(const void *, spid_vec.data()), sizeof(quote_args.spid));
  sofaenclave::occlum::IasReport ias_report;
  int ret = GetQuoteAndFetchIasReport(ias_server, &quote_args, &ias_report);
  if (ret) {
    printf("Fail to get quote or fetch report, erro code is %x!\n", ret);
  } else {
    printf("Test getting quote and fetching report successfully!\n");
  }
  return ret;
}
