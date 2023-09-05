#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <fcntl.h>
#include <unistd.h>

#include "sgx_quote_3.h"

void dump_quote_info(sgx_quote3_t *p_quote)
{
    unsigned int i;
    sgx_report_body_t *p_rep_body;
    sgx_report_data_t *p_rep_data;
    sgx_ql_auth_data_t *p_auth_data;
    sgx_ql_ecdsa_sig_data_t *p_sig_data;
    sgx_ql_certification_data_t *p_cert_data;
    uint64_t *pll;

    p_rep_body = (sgx_report_body_t *)(&p_quote->report_body);
    p_rep_data = (sgx_report_data_t *)(&p_rep_body->report_data);
    p_sig_data = (sgx_ql_ecdsa_sig_data_t *)p_quote->signature_data;
    p_auth_data = (sgx_ql_auth_data_t*)p_sig_data->auth_certification_data;
    p_cert_data = (sgx_ql_certification_data_t *)((uint8_t *)p_auth_data + sizeof(*p_auth_data) + p_auth_data->size);

    printf("cert_key_type = 0x%x\n", p_cert_data->cert_key_type);
    printf("isv product id = %d\n", p_rep_body->isv_prod_id);
    printf("isv svn = %d\n", p_rep_body->isv_svn);

    printf("\nSGX ISV Family ID:\n");
    pll = (uint64_t *)p_rep_body->isv_family_id;
    printf("\tLow 8 bytes: \t0x%08lx\n", *pll++);
    printf("\tHigh 8 bytes: \t0x%08lx\n", *pll);

    printf("\nSGX ISV EXT Product ID:\n");
    pll = (uint64_t *)p_rep_body->isv_ext_prod_id;
    printf("\tLow 8 bytes: \t0x%08lx\n", *pll++);
    printf("\tHigh 8 bytes: \t0x%08lx\n", *pll);

    printf("\nSGX CONFIG ID:");
    for (i = 0; i < SGX_CONFIGID_SIZE; i++) {
        if (!(i % 16))
            printf("\n\t");
        printf("%02x ", p_rep_body->config_id[i]);
    }

    printf("\n\nSGX CONFIG SVN:\n");
    printf("\t0x%04x\n", p_rep_body->config_svn);
}

void main() {
    sgx_quote3_t *p_quote;
    sgx_report_body_t *p_rep_body;
    sgx_report_data_t *p_rep_data;

    // write customer's report data
    char *file_path = "/dev/attestation_report_data";
    char *report_string = "Example Occlum attestation";
    int fd = open(file_path, O_RDWR);
    if (fd < 0) {
        printf("failed to open a file to write");
        return;
    }

    int len = write(fd, report_string, strlen(report_string));
    if (len < 0) {
        printf("failed to write to %d", fd);
        return;
    }

    len = 64;
    char report_data[64] = {0};
    len = read(fd, report_data, len);
    if (len < 0) {
        printf("failed to read from %s", file_path);
        return;
    }

    if (strncmp(report_string, report_data, strlen(report_string)) != 0 ) {
        printf("Read report data is not %s", report_string);
        return;
    }

    close(fd);

    // Generate dcap quote
    fd = open("/dev/attestation_quote", O_RDONLY);
    if (fd < 0) {
        printf("failed to open a file to read");
        return;
    }

    len = 5000;
    char quote_buf[5000] = {0};
    len = read(fd, quote_buf, len);
    if (len < 0) {
        printf("failed to read from /dev/attestation_quote");
        return;
    }

    close(fd);
    printf("DCAP generate quote successfully\n");

    p_quote = (sgx_quote3_t *)quote_buf;
    p_rep_body = (sgx_report_body_t *)(&p_quote->report_body);
    p_rep_data = (sgx_report_data_t *)(&p_rep_body->report_data);

    if (memcmp((void *)p_rep_data, (void *)report_data, sizeof(sgx_report_data_t)) != 0) {
        printf("mismatched report data\n");
        return;
    }

    // Parse the dcap quote
    dump_quote_info(p_quote);

    // Save the quote to host
    fd = open("/host/dcap_quote", O_RDWR | O_CREAT);
    if (fd < 0) {
        printf("failed to open a file to write");
        return;
    }

    len = write(fd, quote_buf, len);
    if (len < 0) {
        printf("failed to write to %d", fd);
        return;
    }

    close(fd);
}
