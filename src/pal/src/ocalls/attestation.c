#include <sgx_uae_service.h>
#include "ocalls.h"

sgx_status_t occlum_ocall_sgx_init_quote(
    sgx_target_info_t *target_info,
    sgx_epid_group_id_t *epid_group_id) {
    // Intel's manual:
    // It's suggested that the caller should wait (typically several seconds
    // to ten of seconds) and retry this API if SGX_ERROR_BUSY is returned.
    return sgx_init_quote(target_info, epid_group_id);
}

sgx_status_t occlum_ocall_sgx_get_quote(
    uint8_t *sigrl,
    uint32_t sigrl_len,
    sgx_report_t *report,
    sgx_quote_sign_type_t quote_type,
    sgx_spid_t *spid,
    sgx_quote_nonce_t *nonce,
    sgx_report_t *qe_report,
    sgx_quote_t *quote_buf,
    uint32_t quote_buf_len) {
    sgx_status_t ret = SGX_SUCCESS;

    uint32_t real_quote_len;
    ret = sgx_calc_quote_size(sigrl, sigrl_len, &real_quote_len);
    if (ret != SGX_SUCCESS) {
        return ret;
    }
    if (quote_buf_len < real_quote_len) {
        return SGX_ERROR_INVALID_PARAMETER;
    }

    // Intel's manual:
    // It's suggested that the caller should wait (typically several seconds
    // to ten of seconds) and retry this API if SGX_ERROR_BUSY is returned.
    ret = sgx_get_quote(report,
                        quote_type,
                        spid,
                        nonce,
                        sigrl,
                        sigrl_len,
                        qe_report,
                        quote_buf,
                        real_quote_len);
    return ret;
}
