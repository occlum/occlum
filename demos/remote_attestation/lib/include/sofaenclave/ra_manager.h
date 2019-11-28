#ifndef REMOTE_ATTESTATION_LIB_INCLUDE_RA_MANAGER_H_
#define REMOTE_ATTESTATION_LIB_INCLUDE_RA_MANAGER_H_

#include <string>

#include "./sgx_quote.h"
#include "./sgx_report.h"
#include "./sgx_tseal.h"
#include "./sgx_urts.h"

#include "sofaenclave/common/error.h"
#include "sofaenclave/common/type.h"

#include "./ra_report.h"

#ifdef __cplusplus
extern "C" {
#endif

/**
 * @brief Initialization for getting enclave quote
 * @param gid   return GID for getting SigRL from attestation server
 * @return Function run successfully or failed
 *   @retval 0 on success
 *   @retval Others when failed
 */
SofaeErrorCode InitializeQuote(sgx_epid_group_id_t* gid);

/**
 * @brief Get enclave quote for remote attestation
 * @param quote_args    All the input parameters required by get quote function.
 *                      The output buffer is also in this structure. Please
 *                      refer to the description of it in type.h header file.
 * @return Function run successfully or failed
 *   @retval 0 on success
 *   @retval Others when failed
 */
SofaeErrorCode GetQuote(SofaeQuoteArgs* quote_args);

/**
 * @brief Fetch IAS report after ge.
 * @param ias_server    Specify the IAS server address, certificate and key.
 *                      If HTTP proxy server is used, certificate and key are
 *                      optional.
 * @param gid           input GID for getting SigRL from attestation server
 * @param sigrl         The string including the response from IAS
 * @return Function run successfully or failed
 *   @retval 0 on success
 *   @retval Others when failed
 */
SofaeErrorCode FetchIasSigRL(const SofaeServerCfg& ias_server,
                             sgx_epid_group_id_t* gid,
                             std::string* sigrl);

/**
 * @brief Fetch IAS report after get quote by GetQuote() function.
 * @param ias_server    Specify the IAS server address, certificate and key.
 *                      If HTTP proxy server is used, certificate and key are
 *                      optional.
 * @param quote         The input quote data returned by GetQuote() function
 * @param ias_report    The output IAS report strings wrapped by IasReport
 * @return Function run successfully or failed
 *   @retval 0 on success
 *   @retval Others when failed
 */
SofaeErrorCode FetchIasReport(const SofaeServerCfg& ias_server,
                              sgx_quote_t* quote,
                              SofaeIasReport* ias_report);

/**
 * @brief All together to initialize quote, get quote and then fetch IAS report.
 * @param ias_server    Specify the IAS server address, certificate and key.
 *                      If HTTP proxy server is used, certificate and key are
 *                      optional.
 * @param quote_args    All the input parameters required by get quote function.
 *                      The output buffer is also in this structure. Please
 *                      refer to the description of it in type.h header file.
 * @param ias_report    The output IAS report strings wrapped by IasReport
 * @return Function run successfully or failed
 *   @retval 0 on success
 *   @retval Others when failed
 */
SofaeErrorCode GetQuoteAndFetchIasReport(const SofaeServerCfg& ias_server,
                                         SofaeQuoteArgs* quote_args,
                                         SofaeIasReport* ias_report);

#ifdef __cplusplus
}
#endif

#endif  // REMOTE_ATTESTATION_LIB_INCLUDE_RA_MANAGER_H_
