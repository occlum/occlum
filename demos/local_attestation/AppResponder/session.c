#include <string.h>
#include <stdio.h>
#include "session.h"

#define MAX_SESSION_COUNT  16
typedef struct _session_id_tracker_t {
    uint32_t          session_id;
} session_id_tracker_t;

int generate_session_id(uint32_t *session_id);
int is_session_id_valid(uint32_t session_id);

session_id_tracker_t *g_session_id_tracker[MAX_SESSION_COUNT];
dh_session_t g_dest_session_info_map[MAX_SESSION_COUNT];
uint32_t g_session_count = 0;
int verify_peer_enclave_trust(sgx_dh_session_enclave_identity_t *peer_enclave_identity) {
    if (!peer_enclave_identity) {
        return ERROR;
    }

    if (!(peer_enclave_identity->attributes.flags & SGX_FLAGS_INITTED)) {
        return ERROR;
    }

    return SUCCESS;
}
int  session_request(sgx_dh_msg1_t *dh_msg1,
                     uint32_t *session_id ) {
    sgx_dh_session_t sgx_dh_session;
    sgx_status_t status = SGX_SUCCESS;

    if (!session_id || !dh_msg1) {
        return ERROR;
    }
    //Intialize the session as a session responder
    status = sgx_dh_init_session(SGX_DH_SESSION_RESPONDER, &sgx_dh_session);
    if (SGX_SUCCESS != status) {
        printf("sgx_dh_init_session failed\n");
        return ERROR;
    }

    //get a new SessionID
    if (generate_session_id(session_id) < 0) {
        return ERROR;    //no more sessions available
    }

    //Allocate memory for the session id tracker
    g_session_id_tracker[*session_id] = (session_id_tracker_t *)malloc(sizeof(
                                            session_id_tracker_t));
    if (!g_session_id_tracker[*session_id]) {
        printf("g_session_id_tracker  failed\n");
        return ERROR;
    }

    memset(g_session_id_tracker[*session_id], 0, sizeof(session_id_tracker_t));
    g_session_id_tracker[*session_id]->session_id = *session_id;

    //Generate Message1 that will be returned to Source Enclave
    status = sgx_dh_responder_gen_msg1((sgx_dh_msg1_t *)dh_msg1, &sgx_dh_session);
    if (SGX_SUCCESS != status) {
        free(g_session_id_tracker[*session_id]);
        printf("sgx_dh_responder_gen_msg1  failed\n");
        return ERROR;
    }
    memcpy(&g_dest_session_info_map[*session_id].in_progress.dh_session, &sgx_dh_session,
           sizeof(sgx_dh_session_t));
    //Store the session information under the correspoding source enlave id key
    g_dest_session_info_map[*session_id].status = IN_PROGRESS;

    return SUCCESS;
}
int exchange_report(sgx_dh_msg2_t *dh_msg2,
                    sgx_dh_msg3_t *dh_msg3,
                    uint32_t session_id) {
    sgx_key_128bit_t dh_aek;   // Session key
    dh_session_t *session_info;
    int  status = SUCCESS;
    sgx_dh_session_t sgx_dh_session;
    sgx_dh_session_enclave_identity_t initiator_identity;

    if (!dh_msg2 || !dh_msg3 || !is_session_id_valid(session_id)) {
        return ERROR;
    }

    memset(&dh_aek, 0, sizeof(sgx_key_128bit_t));
    do {
        //Retreive the session information for the corresponding source enclave id
        session_info  = &g_dest_session_info_map[session_id];

        memcpy(&sgx_dh_session, &session_info->in_progress.dh_session, sizeof(sgx_dh_session_t));

        dh_msg3->msg3_body.additional_prop_length = 0;
        //Process message 2 from source enclave and obtain message 3
        sgx_status_t se_ret = sgx_dh_responder_proc_msg2(dh_msg2,
                              dh_msg3,
                              &sgx_dh_session,
                              &dh_aek,
                              &initiator_identity);
        if (SGX_SUCCESS != se_ret) {
            status = ERROR;
            printf("sgx_dh_responder_proc_msg2 failed\n");
            break;
        }

        //Verify source enclave's trust
        if (verify_peer_enclave_trust(&initiator_identity) != SUCCESS) {
            status = ERROR;
            break;
        }

        //save the session ID, status and initialize the session nonce
        session_info->session_id = session_id;
        session_info->status = ACTIVE;
        session_info->active.counter = 0;
        memcpy(session_info->active.AEK, &dh_aek, sizeof(sgx_key_128bit_t));
        memset(&dh_aek, 0, sizeof(sgx_key_128bit_t));
        g_session_count++;
    } while (0);

    if (status != SUCCESS) {
        end_session(session_id);
    }

    return status;
}

int end_session(uint32_t session_id) {
    int status = SUCCESS;
    int i;
    dh_session_t *session_info;
    if (!is_session_id_valid(session_id)) { return ERROR; }
    //Get the session information from the map corresponding to the source enclave id
    session_info = &g_dest_session_info_map[session_id];

    //Erase the session information for the current session
    //Update the session id tracker
    if (g_session_count > 0) {
        //check if session exists
        for (i = 1; i <= MAX_SESSION_COUNT; i++) {
            if (g_session_id_tracker[i - 1] != NULL &&
                    g_session_id_tracker[i - 1]->session_id == session_id) {
                memset(g_session_id_tracker[i - 1], 0, sizeof(session_id_tracker_t));
                free(g_session_id_tracker[i - 1]);
                g_session_count--;
                break;
            }
        }
    }
    return status;
}

int is_session_id_valid(uint32_t session_id) {
    if (session_id >= MAX_SESSION_COUNT || session_id < 0) { return 0; }
    return (g_session_id_tracker[session_id] != NULL);

}

dh_session_t *get_session_info(uint32_t session_id ) {
    if (!is_session_id_valid(session_id)) {
        return NULL;
    }
    return &(g_dest_session_info_map[session_id]);

}

int generate_session_id(uint32_t *session_id) {
    if (!session_id) {
        return ERROR;
    }
    //if the session structure is untintialized, set that as the next session ID
    for (int i = 0; i < MAX_SESSION_COUNT; i++) {
        if (g_session_id_tracker[i] == NULL) {
            *session_id = i;
            return SUCCESS;
        }
    }
    return ERROR;
}
