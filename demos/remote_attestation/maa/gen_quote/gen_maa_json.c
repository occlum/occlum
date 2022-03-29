#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <openssl/sha.h>

#include "sgx_quote_3.h"
#include "occlum_dcap.h"

#define MAA_JSON "/host/maa.json"

void sha256sum(const uint8_t *data, uint32_t data_size, uint8_t *hash)
{
    SHA256_CTX sha256;
    SHA256_Init(&sha256);
    SHA256_Update(&sha256, data, data_size);
    SHA256_Final(hash, &sha256);
}

const char *uint16_to_buffer (char *buffer, unsigned int maxSize, uint16_t n, size_t size)
{
    if (size * 2 >= maxSize || size < 2)
        return "DEADBEEF";
    sprintf(&buffer[0], "%02X", (uint8_t)(n));
    sprintf(&buffer[2], "%02X", (uint8_t)(n >> 8));

    for (int i=2; i < size; i++)
    {
        sprintf(&buffer[i*2], "%02X", 0);
    }
    buffer[size*2+1] = '\0';
    return buffer;
}

const char *format_hex_buffer (char *buffer, unsigned int maxSize, uint8_t *data, size_t size)
{
    if (size * 2 >= maxSize)
        return "DEADBEEF";

    for (int i=0; i < size; i++)
    {
        sprintf(&buffer[i*2], "%02X", data[i]);
    }
    buffer[size*2+1] = '\0';
    return buffer;
}


void main() {
    void *handle;
    uint32_t quote_size;
    uint8_t *p_quote_buffer;
    sgx_quote3_t *p_quote;
    sgx_report_body_t *p_report_body;
    sgx_report_data_t *p_report_data;
    int32_t ret;

    const int hex_buffer_size = 1024*64;
    char hex_buffer[hex_buffer_size];
    
    handle = dcap_quote_open();
    quote_size = dcap_get_quote_size(handle);
    printf("quote size = %d\n", quote_size);

    p_quote_buffer = (uint8_t*)malloc(quote_size);
    if (NULL == p_quote_buffer) {
        printf("Couldn't allocate quote_buffer\n");
        goto CLEANUP;
    }
    memset(p_quote_buffer, 0, quote_size);

    uint8_t enclave_held_data[6] = {0x01, 0x02, 0x03, 0x04, 0x05, 0x06};
    sgx_report_data_t hash = {0};
    sha256sum(enclave_held_data, 6, hash.d);
    // printf("report data hash:\n");
    // for (int i = 0; i < sizeof(hash.d); i++) {
    //     if (!(i % 16))
    //         printf("\n\t");
    //     printf("%02x ", hash.d[i]);
    // }
    // printf("\n");

    // Get the Quote
    ret = dcap_generate_quote(handle, p_quote_buffer, &hash);
    if (0 != ret) {
        printf( "Error in dcap_generate_quote.\n");
        goto CLEANUP;
    }

    printf("DCAP generate quote successfully\n");

    p_quote = (sgx_quote3_t *)p_quote_buffer;
    p_report_body = (sgx_report_body_t *)(&p_quote->report_body);
    p_report_data = (sgx_report_data_t *)(&p_report_body->report_data);

    // check report data
    if (memcmp((void *)&hash, (void *)p_report_data, sizeof(sgx_report_data_t)) != 0) {
        printf("mismathced report data\n");
        goto CLEANUP;
    }

    // Create json file
    FILE *fp = fopen(MAA_JSON, "w");
    if ( NULL == fp ) {
        printf("fopen %s failed return %d\n", MAA_JSON, errno);
        goto CLEANUP;
    }

    // Generate Azure attestion json file
    // Refer to https://github.com/Azure-Samples/microsoft-azure-attestation
    fprintf(fp, "%s\n", "{");
    // Use 3 as type for now
    fprintf(fp, "  \"Type\": %d,\n", 3);
    fprintf(fp, "  \"MrEnclaveHex\": \"%s\",\n", format_hex_buffer(hex_buffer, hex_buffer_size, p_report_body->mr_enclave.m, SGX_HASH_SIZE));
    fprintf(fp, "  \"MrSignerHex\": \"%s\",\n", format_hex_buffer(hex_buffer, hex_buffer_size, p_report_body->mr_signer.m, SGX_HASH_SIZE));
    fprintf(fp, "  \"ProductIdHex\": \"%s\",\n", uint16_to_buffer(hex_buffer, hex_buffer_size, (uint16_t)p_report_body->isv_prod_id, 16));
    fprintf(fp, "  \"SecurityVersion\": %u,\n", (int)p_report_body->isv_svn);
    fprintf(fp, "  \"Attributes\": %lu,\n", (uint64_t)p_report_body->attributes.flags);
    fprintf(fp, "  \"QuoteHex\": \"%s\",\n", format_hex_buffer(hex_buffer, hex_buffer_size, (uint8_t *)p_quote, quote_size));
    fprintf(fp, "  \"EnclaveHeldDataHex\": \"%s\"\n", format_hex_buffer(hex_buffer, hex_buffer_size, enclave_held_data, sizeof( enclave_held_data)));
    fprintf(fp, "%s\n", "}");
    fclose(fp);

CLEANUP:
    if (NULL != p_quote_buffer) {
        free(p_quote_buffer);
    }

    dcap_quote_close(handle);
}
