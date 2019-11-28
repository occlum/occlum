#ifndef REMOTE_ATTESTATION_LIB_INCLUDE_RA_DEVICE_H_
#define REMOTE_ATTESTATION_LIB_INCLUDE_RA_DEVICE_H_

#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <sys/ioctl.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <termios.h>
#include <unistd.h>

#include <string>

#include "sofaenclave/common/error.h"
#include "sofaenclave/common/type.h"

namespace sofaenclave {
namespace occlum {

SofaeErrorCode SgxDeviceGetGroupID(sgx_epid_group_id_t* gid);
SofaeErrorCode SgxDeviceGetQuote(SofaeQuoteArgs* quote_args);

}  // namespace occlum
}  // namespace sofaenclave

#endif  // REMOTE_ATTESTATION_LIB_INCLUDE_RA_DEVICE_H_
