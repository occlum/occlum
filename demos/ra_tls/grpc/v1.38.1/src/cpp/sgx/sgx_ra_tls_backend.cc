#include "sgx_ra_tls_utils.h"
#include "sgx_ra_tls_backend.h"
#include "sgx_quote_3.h"
#include "occlum_dcap.h"

namespace grpc {
namespace sgx {
int verify_quote (uint8_t * quote_buffer, size_t quote_size) {
    void *handle;
    handle = dcap_quote_open();
    uint32_t supplemental_size, ret;
    uint8_t *p_supplemental_buffer;
    sgx_ql_qv_result_t quote_verification_result = SGX_QL_QV_RESULT_UNSPECIFIED;
    uint32_t collateral_expiration_status = 1;
    supplemental_size = dcap_get_supplemental_data_size(handle);
    p_supplemental_buffer = (uint8_t *)malloc(supplemental_size);
    if (NULL == p_supplemental_buffer) {
        printf("Couldn't allocate supplemental buffer\n");
    }
    memset(p_supplemental_buffer, 0, supplemental_size);
    ret = dcap_verify_quote(
        handle,
        quote_buffer,
        quote_size,
        &collateral_expiration_status,
        &quote_verification_result,
        supplemental_size,
        p_supplemental_buffer
        );
    
    if (0 != ret) {
        printf( "Error in dcap_verify_quote.\n");
    }

    if (collateral_expiration_status != 0) {
        printf("the verification collateral has expired\n");
    }
    dcap_quote_close(handle);
}

int generate_quote(uint8_t *quote_buffer, unsigned char *hash, size_t hash_len) {
  void *handle;

  handle = dcap_quote_open();


  sgx_report_data_t report_data = { 0 };
  memcpy(report_data.d, hash, hash_len);

  // Get the Quote
  int ret = dcap_generate_quote(handle, quote_buffer, &report_data);
  if (0 != ret) {
    printf( "Error in dcap_generate_quote.\n");
  }
 
  dcap_quote_close(handle);
  return ret;
}

uint32_t get_quote_size() {
  void *handle = dcap_quote_open();
  uint32_t quote_size = dcap_get_quote_size(handle);
  dcap_quote_close(handle);
  return quote_size;
}

}//namespace grpc
}//namesapce sgx
