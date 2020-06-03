#ifndef _EC_DH_SESSION_H_
#define _EC_DH_SESSION_H_

#define ERROR    -1
#define SUCCESS  0

#define CLOSED 0x0
#define IN_PROGRESS 0x1
#define ACTIVE 0x2

#include "sgx_dh.h"
#include "dh_session_protocol.h"

int session_request(sgx_dh_msg1_t *dh_msg1,
                    uint32_t *session_id );
int exchange_report(sgx_dh_msg2_t *dh_msg2,
                    sgx_dh_msg3_t *dh_msg3,
                    uint32_t session_id);
int end_session(uint32_t session_id);
dh_session_t *get_session_info(uint32_t session_id );

#endif
