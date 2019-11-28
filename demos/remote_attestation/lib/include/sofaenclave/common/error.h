#ifndef REMOTE_ATTESTATION_LIB_INCLUDE_SOFAENCLAVE_COMMON_ERROR_H_
#define REMOTE_ATTESTATION_LIB_INCLUDE_SOFAENCLAVE_COMMON_ERROR_H_

/* We want to use SofaeErrorCode to include the error code from Intel SDK and
 * also the SOFAEnclave code, so we use the same unsigned int type here.*/
typedef int SofaeErrorCode;

#define SOFAE_MK_ERROR(x)                          (0xFFFF0000&((x) << 16))

#define SOFAE_SUCCESS                              (0x00000000)

#define SOFAE_ERROR_GENERIC                        SOFAE_MK_ERROR(0x0001)
#define SOFAE_ERROR_PARAMETERS                     SOFAE_MK_ERROR(0x0002)
#define SOFAE_ERROR_MALLOC                         SOFAE_MK_ERROR(0x0003)
#define SOFAE_ERROR_ENCLAVE_NOTINITIALIZED         SOFAE_MK_ERROR(0x0004)
#define SOFAE_ERROR_REPORT_DATA_SIZE               SOFAE_MK_ERROR(0x0005)
#define SOFAE_ERROR_PARSE_CONFIGURATIONS           SOFAE_MK_ERROR(0x0006)
#define SOFAE_ERROR_PARSE_COMMANDLINE              SOFAE_MK_ERROR(0x0007)

#define SOFAE_ERROR_FILE_OPEN                      SOFAE_MK_ERROR(0x0101)
#define SOFAE_ERROR_FILE_READ                      SOFAE_MK_ERROR(0x0102)
#define SOFAE_ERROR_FILE_WRITE                     SOFAE_MK_ERROR(0x0103)

#define SOFAE_ERROR_CONF_LOAD                      SOFAE_MK_ERROR(0x0201)
#define SOFAE_ERROR_CONF_NOTEXIST                  SOFAE_MK_ERROR(0x0202)

#define SOFAE_ERROR_IAS_CLIENT_INIT                SOFAE_MK_ERROR(0x0501)
#define SOFAE_ERROR_IAS_CLIENT_CONNECT             SOFAE_MK_ERROR(0x0502)
#define SOFAE_ERROR_IAS_CLIENT_GETSIGRL            SOFAE_MK_ERROR(0x0503)
#define SOFAE_ERROR_IAS_CLIENT_GETREPORT           SOFAE_MK_ERROR(0x0504)
#define SOFAE_ERROR_IAS_CLIENT_UNESCAPE            SOFAE_MK_ERROR(0x0505)
#define SOFAE_ERROR_IAS_LOAD_CACHED_REPORT         SOFAE_MK_ERROR(0x0506)

#define SOFAE_ERROR_SDK_UNEXPECTED                 SOFAE_MK_ERROR(0x0FFF)

#define SOFAE_ERROR_CODE(rc) (rc)
#define SOFAE_ERROR_MERGE(ecallcode, retcode) ((ecallcode) | (retcode))

#endif  // REMOTE_ATTESTATION_LIB_INCLUDE_SOFAENCLAVE_COMMON_ERROR_H_
