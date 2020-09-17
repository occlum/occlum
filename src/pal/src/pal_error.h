#ifndef __PAL_ERROR_H__
#define __PAL_ERROR_H__

#include <errno.h>
#include <sgx_error.h>

const char *pal_get_sgx_error_msg(sgx_status_t error);

#endif /* __PAL_ERROR_H__ */
