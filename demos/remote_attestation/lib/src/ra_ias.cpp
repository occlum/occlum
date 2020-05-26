#include <cstring>
#include <string>
#include <vector>

#include "sofaenclave/common/error.h"
#include "sofaenclave/common/log.h"
#include "sofaenclave/common/type.h"
#include "sofaenclave/ra_json.h"
#include "sofaenclave/ra_ias.h"

// use cppcodec/base64
#include "cppcodec/base64_rfc4648.hpp"
using base64 = cppcodec::base64_rfc4648;

namespace sofaenclave {
namespace occlum {

constexpr char kStrEpidPseudonym[] = "epidPseudonym";
constexpr char kStrQuoteStatus[] = "isvEnclaveQuoteStatus";
constexpr char kStrPlatform[] = "platformInfoBlob";
constexpr char kStrQuoteBody[] = "isvEnclaveQuoteBody";
constexpr char kStrHeaderSig[] = "x-iasreport-signature:";
constexpr char kStrHeaderSigAk[] = "X-IASReport-Signature:";
constexpr char kStrHeaderCa[] = "x-iasreport-signing-certificate:";
constexpr char kStrHeaderCaAk[] = "X-IASReport-Signing-Certificate:";
constexpr char kStrHeaderAdvisoryUrl[] = "advisory-url:";
constexpr char kStrHeaderAdvisoryIDs[] = "advisory-ids:";

typedef struct {
  std::string b64_sigrl;
} SofaeIasSigrl;

static std::string GetHeaderValue(const char *header, const char *name) {
  std::string header_str = header;
  std::string ending("\n\r");

  // Name: value\r\n
  std::size_t pos_start = header_str.find_first_of(" ");
  std::size_t pos_end = header_str.find_first_of("\r\n");
  if ((pos_start != std::string::npos) && (pos_end != std::string::npos)) {
    return header_str.substr(pos_start + 1, pos_end - pos_start - 1);
  } else {
    return std::string("");
  }
}

static size_t ParseSigrlResponseBody(const void* contents, size_t size,
                                     size_t nmemb, void* response) {
  size_t content_length = size * nmemb;
  SofaeIasSigrl *sigrl = RCAST(SofaeIasSigrl *, response);

  if (content_length == 0) {
    sigrl->b64_sigrl.clear();
    SOFAE_LOG_DEBUG("GetSigRL: Empty");
  } else {
    sigrl->b64_sigrl.assign(RCAST(const char *, contents), content_length);
    SOFAE_LOG_DEBUG("GetSigRL: %s", sigrl->b64_sigrl.c_str());
  }
  return content_length;
}

static size_t ParseSigrlResponseHeader(const void* contents, size_t size,
                                       size_t nmemb, void* response) {
  size_t len = size * nmemb;
  const char *header = RCAST(const char *, contents);

  SOFAE_LOG_DEBUG("IAS Get SigRL %s", header);
  return len;
}

static size_t ParseReportResponseBody(const void* contents, size_t size,
                                      size_t nmemb, void* response) {
  const char *body = RCAST(const char *, contents);
  size_t content_length = size * nmemb;
  IasReport *report = RCAST(IasReport *, response);

  report->set_response_body(body, content_length);

  rapidjson::Document doc;
  if (doc.Parse(body).HasParseError()) {
    SOFAE_LOG_ERROR("Fail to parse report response body");
  } else {
    report->set_epid_pseudonym(JsonConfig::GetStr(doc, kStrEpidPseudonym));
    report->set_quote_status(JsonConfig::GetStr(doc, kStrQuoteStatus));
    report->set_b16_platform_info_blob(JsonConfig::GetStr(doc, kStrPlatform));
    report->set_b64_quote_body(JsonConfig::GetStr(doc, kStrQuoteBody));
  }

  return content_length;
}

static size_t ParseReportResponseHeader(const void* contents, size_t size,
                                        size_t nmemb, void* response) {
  size_t len = size * nmemb;
  const char *header = RCAST(const char *, contents);
  IasReport *report = RCAST(IasReport *, response);

  if (strncmp(header, kStrHeaderSig, strlen(kStrHeaderSig)) == 0) {
    report->set_b64_signature(GetHeaderValue(header, kStrHeaderSig));
  } else if (strncmp(header, kStrHeaderSigAk, strlen(kStrHeaderSigAk)) == 0) {
    report->set_b64_signature(GetHeaderValue(header, kStrHeaderSigAk));
  } else if (strncmp(header, kStrHeaderCa, strlen(kStrHeaderCa)) == 0) {
    report->set_signing_cert(GetHeaderValue(header, kStrHeaderCa));
  } else if (strncmp(header, kStrHeaderCaAk, strlen(kStrHeaderCaAk)) == 0) {
    report->set_signing_cert(GetHeaderValue(header, kStrHeaderCaAk));
  } else if (strncmp(header, kStrHeaderAdvisoryUrl,
                     strlen(kStrHeaderAdvisoryUrl)) == 0) {
    report->set_advisory_url(GetHeaderValue(header, kStrHeaderAdvisoryUrl));
  } else if (strncmp(header, kStrHeaderAdvisoryIDs,
                     strlen(kStrHeaderAdvisoryIDs)) == 0) {
    report->set_advisory_ids(GetHeaderValue(header, kStrHeaderAdvisoryIDs));
  }
  return len;
}

std::mutex RaIasClient::init_mutex_;

void RaIasClient::InitIasConnection(const std::string& endpoint) {
  if (endpoint.empty()) {
    curl_ = NULL;
    return;
  }

  // curl_global_init is not multithreads safe function. It's suggested to
  // call it in main thread. Here we just add lock to make sure safety, but
  // don't consider the performance, as multithreads is not common usecase.
  {
    std::lock_guard<std::mutex> lock(init_mutex_);
    curl_global_init(CURL_GLOBAL_ALL);
  }

  curl_ = curl_easy_init();
  if (!curl_) {
    return;
  }

#ifdef DEBUG
  /* set libcurl verbose */
  curl_easy_setopt(curl_, CURLOPT_VERBOSE, 1L);
#endif

  /* set the common header */
  headers_ = curl_slist_append(NULL, "Accept: application/json");
  headers_ = curl_slist_append(headers_, "Content-Type: application/json");
  curl_easy_setopt(curl_, CURLOPT_HTTPHEADER, headers_);
  curl_easy_setopt(curl_, CURLOPT_USERAGENT, "sgx-sp/1.0");

  /* set commom option */
  curl_easy_setopt(curl_, CURLOPT_FORBID_REUSE, 1L);
  curl_easy_setopt(curl_, CURLOPT_NOSIGNAL, 1L);
  curl_easy_setopt(curl_, CURLOPT_TIMEOUT, 60L);
  curl_easy_setopt(curl_, CURLOPT_CONNECTTIMEOUT, 10L);
  curl_easy_setopt(curl_, CURLOPT_SSL_VERIFYPEER, 0L);
  curl_easy_setopt(curl_, CURLOPT_SSL_VERIFYHOST, 0L);

  server_endpoint_ = endpoint;
}

RaIasClient::RaIasClient(const std::string& endpoint) {
  InitIasConnection(endpoint);
}

RaIasClient::RaIasClient(const SofaeServerCfg& ias_server) {
  // Configure the other normal settings firstly.
  InitIasConnection(ias_server.endpoint);

  // Check the HTTPS server addr and set the cert/key settings
  // Or use the Access key authentication
  std::string header_access_key = "Ocp-Apim-Subscription-Key: ";
  if (!ias_server.accesskey.empty()) {
    header_access_key += ias_server.accesskey;
    headers_ = curl_slist_append(headers_, header_access_key.c_str());
  }

  if (curl_ && (ias_server.endpoint.find("https://") != std::string::npos) && \
      (ias_server.accesskey.empty())) {
    const char *ias_cert_key_type = "PEM";
    SOFAE_LOG_DEBUG("IAS cert: %s", ias_server.cert.c_str());
    SOFAE_LOG_DEBUG("IAS key: %s", ias_server.key.c_str());

    curl_easy_setopt(curl_, CURLOPT_SSLCERT, ias_server.cert.c_str());
    curl_easy_setopt(curl_, CURLOPT_SSLKEY, ias_server.key.c_str());
    curl_easy_setopt(curl_, CURLOPT_SSLCERTTYPE, ias_cert_key_type);
    curl_easy_setopt(curl_, CURLOPT_SSLKEYTYPE, ias_cert_key_type);
  }
}

RaIasClient::~RaIasClient() {
  if (headers_) {
    curl_slist_free_all(headers_);
  }
  if (curl_) {
    curl_easy_cleanup(curl_);
  }

  // add lock for multi-threads safety
  {
    std::lock_guard<std::mutex> lock(init_mutex_);
    curl_global_cleanup();
  }
}

SofaeErrorCode RaIasClient::GetSigRL(const sgx_epid_group_id_t& gid,
                                     std::string *sigrl) {
  if (!curl_) {
    SOFAE_LOG_ERROR("IAS client is not initialized");
    return SOFAE_ERROR_IAS_CLIENT_INIT;
  }

  /* Set the URL */
  std::string url = server_endpoint_ + "/sigrl/";
  std::vector<char> tmp_gid_vec(sizeof(sgx_epid_group_id_t) * 2 + 1, 0);
  snprintf(tmp_gid_vec.data(), tmp_gid_vec.size(), "%02X%02X%02X%02X", gid[3],
           gid[2], gid[1], gid[0]);
  url += std::string(tmp_gid_vec.data());
  SOFAE_LOG_DEBUG("URL: %s", url.c_str());
  curl_easy_setopt(curl_, CURLOPT_URL, url.c_str());

  /* Set the sigrl request header and body handler function and data */
  SofaeIasSigrl ias_sigrl;
  curl_easy_setopt(curl_, CURLOPT_WRITEFUNCTION, ParseSigrlResponseBody);
  curl_easy_setopt(curl_, CURLOPT_HEADERFUNCTION, ParseSigrlResponseHeader);
  curl_easy_setopt(curl_, CURLOPT_WRITEDATA, RCAST(void *, &ias_sigrl));
  curl_easy_setopt(curl_, CURLOPT_WRITEHEADER, RCAST(void *, &ias_sigrl));

  CURLcode rc = curl_easy_perform(curl_);
  if (rc != CURLE_OK) {
    SOFAE_LOG_ERROR("Fail to connect server: %s\n", curl_easy_strerror(rc));
    return SOFAE_ERROR_IAS_CLIENT_CONNECT;
  }

  if (!ias_sigrl.b64_sigrl.empty()) {
    std::vector<uint8_t> sigrl_vec;
    try {
      sigrl_vec = base64::decode(ias_sigrl.b64_sigrl);
    } catch (std::exception& e) {
      SOFAE_LOG_ERROR("Cannot decode base64 sigrl: %s", e.what());
      return SOFAE_ERROR_IAS_CLIENT_GETSIGRL;
    }
    sigrl->assign(RCAST(const char *, sigrl_vec.data()), sigrl_vec.size());
  }
  return SOFAE_SUCCESS;
}

SofaeErrorCode RaIasClient::FetchReport(const std::string& quote,
                                        IasReport *ias_report) {
  /* should not be empty is not to use cache */
  if (quote.empty()) {
    SOFAE_LOG_ERROR("Invalid base64 quote value");
    return SOFAE_ERROR_PARAMETERS;
  }

  if (!curl_) {
    SOFAE_LOG_ERROR("IAS client is not initialized!");
    return SOFAE_ERROR_IAS_CLIENT_INIT;
  }

  /* Set the report url */
  std::string url = server_endpoint_ + "/report";
  SOFAE_LOG_DEBUG("URL: %s", url.c_str());
  curl_easy_setopt(curl_, CURLOPT_URL, url.c_str());

  /* Set the post data */
  SOFAE_LOG_DEBUG("Quote length: %ld", quote.length());
  std::string b64_quote = base64::encode(RCAST(const char *, quote.c_str()),
                                         SCAST(size_t, quote.length()));
  SOFAE_LOG_DEBUG("QUTEO[%lu]: %s", b64_quote.length(), b64_quote.c_str());
  std::string post_data = "{\"isvEnclaveQuote\": \"";
  post_data += b64_quote;
  post_data += "\"}";
  curl_easy_setopt(curl_, CURLOPT_POSTFIELDS, post_data.c_str());

  /* Set the report request header and body handler function and data */
  curl_easy_setopt(curl_, CURLOPT_WRITEFUNCTION, ParseReportResponseBody);
  curl_easy_setopt(curl_, CURLOPT_HEADERFUNCTION, ParseReportResponseHeader);
  curl_easy_setopt(curl_, CURLOPT_WRITEDATA, RCAST(void *, ias_report));
  curl_easy_setopt(curl_, CURLOPT_WRITEHEADER, RCAST(void *, ias_report));

  CURLcode rc = curl_easy_perform(curl_);
  if (rc != CURLE_OK) {
    SOFAE_LOG_ERROR("Fail to connect server: %s\n", curl_easy_strerror(rc));
    return SOFAE_ERROR_IAS_CLIENT_CONNECT;
  }

  /* deal with the escaped certificates */
  std::string signing_cert = ias_report->signing_cert();
  if (!signing_cert.empty()) {
    int unescape_len = 0;
    char *p_unescape = curl_easy_unescape(curl_, signing_cert.data(),
                                          signing_cert.length(), &unescape_len);
    if (p_unescape && unescape_len) {
      ias_report->set_signing_cert(p_unescape, unescape_len);
      curl_free(p_unescape);
    } else {
      SOFAE_LOG_ERROR("Fail to convert the escaped certificate in response.");
      return SOFAE_ERROR_IAS_CLIENT_UNESCAPE;
    }
  } else {
    SOFAE_LOG_ERROR("Fail to get quote report from IAS");
    return SOFAE_ERROR_IAS_CLIENT_GETREPORT;
  }

  return SOFAE_SUCCESS;
}

}  // namespace occlum
}  // namespace sofaenclave
