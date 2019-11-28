#ifndef REMOTE_ATTESTATION_LIB_INCLUDE_SOFAENCLAVE_COMMON_LOG_H_
#define REMOTE_ATTESTATION_LIB_INCLUDE_SOFAENCLAVE_COMMON_LOG_H_

#include <string>

extern "C" int printf(const char *fmt, ...);

#ifdef DEBUG
#define SOFAE_LOG_DEBUG(fmt, ...) \
  printf("[DEBUG][%s:%d] " fmt "\n", __FILE__, __LINE__, ##__VA_ARGS__)
#else
#define SOFAE_LOG_DEBUG(fmt, ...)
#endif

#define SOFAE_LOG_INFO(fmt, ...) \
  printf("[INFO][%s:%d] " fmt "\n", __FILE__, __LINE__, ##__VA_ARGS__)
#define SOFAE_LOG_WARN(fmt, ...) \
  printf("[WARN][%s:%d] " fmt "\n", __FILE__, __LINE__, ##__VA_ARGS__)
#define SOFAE_LOG_ERROR(fmt, ...) \
  printf("[ERROR][%s:%d] " fmt "\n", __FILE__, __LINE__, ##__VA_ARGS__)

#endif  // REMOTE_ATTESTATION_LIB_INCLUDE_SOFAENCLAVE_COMMON_LOG_H_
