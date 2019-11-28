#include <string>

#include "sofaenclave/common/error.h"
#include "sofaenclave/common/log.h"
#include "sofaenclave/ra_device.h"

namespace sofaenclave {
namespace occlum {

#define SGXIOC_GET_EPID_GROUP_ID _IOR('s', 1, sgx_epid_group_id_t)
#define SGXIOC_GEN_QUOTE _IOWR('s', 2, SofaeQuoteArgs)

constexpr char kSgxDeviceName[] = "/dev/sgx";

SofaeErrorCode SgxDeviceGetGroupID(sgx_epid_group_id_t* gid) {
  int sgx_fd;
  if ((sgx_fd = open(kSgxDeviceName, O_RDONLY)) < 0) {
    SOFAE_LOG_ERROR("Fail to open %s", kSgxDeviceName);
    return SOFAE_ERROR_FILE_OPEN;
  }

  SofaeErrorCode ret = SOFAE_SUCCESS;
  if (ioctl(sgx_fd, SGXIOC_GET_EPID_GROUP_ID, gid) < 0) {
    SOFAE_LOG_ERROR("Fail to get group id from  %s", kSgxDeviceName);
    ret = SOFAE_ERROR_SDK_UNEXPECTED;
  }

  close(sgx_fd);
  return ret;
}

SofaeErrorCode SgxDeviceGetQuote(SofaeQuoteArgs* quote_args) {
  int sgx_fd;
  if ((sgx_fd = open(kSgxDeviceName, O_RDONLY)) < 0) {
    SOFAE_LOG_ERROR("Fail to open %s", kSgxDeviceName);
    return SOFAE_ERROR_FILE_OPEN;
  }

  SofaeErrorCode ret = SOFAE_SUCCESS;
  int count = 3;
  while (count--) {
    if (ioctl(sgx_fd, SGXIOC_GEN_QUOTE, quote_args) == 0) {
      uint32_t signature_len = quote_args->quote.as_quote->signature_len;
      SOFAE_LOG_DEBUG("SgxDeviceGetQuote length=%ld", signature_len);
      if (signature_len == 0) {
        SOFAE_LOG_ERROR("Invalid quote from %s", kSgxDeviceName);
        ret = SOFAE_ERROR_SDK_UNEXPECTED;
      }
      break;
    }
    else if (errno != EAGAIN) {
      SOFAE_LOG_ERROR("Fail to get quote from %s", kSgxDeviceName);
      ret = SOFAE_ERROR_SDK_UNEXPECTED;
      break;
    }
    else {
      SOFAE_LOG_WARN("/dev/sgx is temporarily busy. Try again after 1s.");
      sleep(1);
    }
  }

  close(sgx_fd);
  return ret;
}

}  // namespace occlum
}  // namespace sofaenclave
