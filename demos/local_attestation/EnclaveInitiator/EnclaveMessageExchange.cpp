/*
 * Copyright (C) 2011-2019 Intel Corporation. All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 *
 *   * Redistributions of source code must retain the above copyright
 *     notice, this list of conditions and the following disclaimer.
 *   * Redistributions in binary form must reproduce the above copyright
 *     notice, this list of conditions and the following disclaimer in
 *     the documentation and/or other materials provided with the
 *     distribution.
 *   * Neither the name of Intel Corporation nor the names of its
 *     contributors may be used to endorse or promote products derived
 *     from this software without specific prior written permission.
 *
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
 * "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
 * LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
 * A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
 * OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
 * SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
 * LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
 * DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
 * THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
 * (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
 * OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 *
 */
#include <string>
#include "sgx_trts.h"
#include "sgx_utils.h"
#include "EnclaveMessageExchange.h"
#include "sgx_eid.h"
#include "error_codes.h"
#include "sgx_ecp_types.h"
#include "sgx_thread.h"
#include "dh_session_protocol.h"
#include "sgx_dh.h"
#include "sgx_tcrypto.h"
#include "EnclaveInitiator_t.h"

#ifdef __cplusplus
extern "C" {
#endif

uint32_t message_exchange_response_generator(char* decrypted_data, char** resp_buffer, size_t* resp_length);
uint32_t verify_peer_enclave_trust(sgx_dh_session_enclave_identity_t* peer_enclave_identity);

#ifdef __cplusplus
}
#endif

#define MAX_SESSION_COUNT  16

//number of open sessions
uint32_t g_session_count = 0;

ATTESTATION_STATUS generate_session_id(uint32_t *session_id);
ATTESTATION_STATUS end_session(sgx_enclave_id_t src_enclave_id);

//Array of open session ids
session_id_tracker_t *g_session_id_tracker[MAX_SESSION_COUNT];

//Create a session with the destination enclave
ATTESTATION_STATUS create_session(dh_session_t *session_info)
{
    sgx_dh_msg1_t dh_msg1;            //Diffie-Hellman Message 1
    sgx_key_128bit_t dh_aek;        // Session Key
    sgx_dh_msg2_t dh_msg2;            //Diffie-Hellman Message 2
    sgx_dh_msg3_t dh_msg3;            //Diffie-Hellman Message 3
    uint32_t session_id;
    uint32_t retstatus;
    sgx_status_t status = SGX_SUCCESS;
    sgx_dh_session_t sgx_dh_session;
    sgx_dh_session_enclave_identity_t responder_identity;

    if(!session_info)
    {
        return INVALID_PARAMETER_ERROR;
    }

    memset(&dh_aek,0, sizeof(sgx_key_128bit_t));
    memset(&dh_msg1, 0, sizeof(sgx_dh_msg1_t));
    memset(&dh_msg2, 0, sizeof(sgx_dh_msg2_t));
    memset(&dh_msg3, 0, sizeof(sgx_dh_msg3_t));
    memset(session_info, 0, sizeof(dh_session_t));

    //Intialize the session as a session initiator
    status = sgx_dh_init_session(SGX_DH_SESSION_INITIATOR, &sgx_dh_session);
    if(SGX_SUCCESS != status)
    {
        return status;
    }

    //Ocall to request for a session with the destination enclave and obtain session id and Message 1 if successful
    status = session_request_ocall(&retstatus, &dh_msg1, &session_id);
    if (status == SGX_SUCCESS)
    {
        if ((ATTESTATION_STATUS)retstatus != SUCCESS)
            return ((ATTESTATION_STATUS)retstatus);
    }
    else
    {
        return ATTESTATION_SE_ERROR;
    }
    //Process the message 1 obtained from desination enclave and generate message 2
    status = sgx_dh_initiator_proc_msg1(&dh_msg1, &dh_msg2, &sgx_dh_session);
    if(SGX_SUCCESS != status)
    {
        return status;
    }

    //Send Message 2 to Destination Enclave and get Message 3 in return
    status = exchange_report_ocall(&retstatus, &dh_msg2, &dh_msg3, session_id);
    if (status == SGX_SUCCESS)
    {
        if ((ATTESTATION_STATUS)retstatus != SUCCESS)
            return ((ATTESTATION_STATUS)retstatus);
    }
    else
    {
        return ATTESTATION_SE_ERROR;
    }

    //Process Message 3 obtained from the destination enclave
    status = sgx_dh_initiator_proc_msg3(&dh_msg3, &sgx_dh_session, &dh_aek, &responder_identity);
    if(SGX_SUCCESS != status)
    {
        return status;
    }

    // Verify the identity of the destination enclave
    if(verify_peer_enclave_trust(&responder_identity) != SUCCESS)
    {
        return INVALID_SESSION;
    }

    memcpy(session_info->active.AEK, &dh_aek, sizeof(sgx_key_128bit_t));
    session_info->session_id = session_id;
    session_info->active.counter = 0;
    session_info->status = ACTIVE;
    memset(&dh_aek,0, sizeof(sgx_key_128bit_t));
    return status;
}

//Request for the response size, send the request message to the destination enclave and receive the response message back
ATTESTATION_STATUS send_request_receive_response(dh_session_t *session_info,
        char *inp_buff,
        size_t inp_buff_len,
        size_t max_out_buff_size,
        char **out_buff,
        size_t* out_buff_len)
{
    const uint8_t* plaintext;
    uint32_t plaintext_length;
    sgx_status_t status;
    uint32_t retstatus;
    secure_message_t* req_message;
    secure_message_t* resp_message;
    uint8_t *decrypted_data;
    uint32_t decrypted_data_length;
    uint32_t plain_text_offset;
    uint8_t l_tag[TAG_SIZE];
    size_t max_resp_message_length;
    plaintext = (const uint8_t*)(" ");
    plaintext_length = 0;

    if(!session_info || !inp_buff)
    {
        return INVALID_PARAMETER_ERROR;
    }
    //Check if the nonce for the session has not exceeded 2^32-2 if so end session and start a new session
    if(session_info->active.counter == ((uint32_t) - 2))
    {
        close_session(session_info);
        create_session(session_info);
    }

    //Allocate memory for the AES-GCM request message
    req_message = (secure_message_t*)malloc(sizeof(secure_message_t)+ inp_buff_len);
    if(!req_message)
        return MALLOC_ERROR;
    memset(req_message, 0, sizeof(secure_message_t)+ inp_buff_len);

    const uint32_t data2encrypt_length = (uint32_t)inp_buff_len;

    //Set the payload size to data to encrypt length
    req_message->message_aes_gcm_data.payload_size = data2encrypt_length;

    //Use the session nonce as the payload IV
    memcpy(req_message->message_aes_gcm_data.reserved, &session_info->active.counter, sizeof(session_info->active.counter));

    //Set the session ID of the message to the current session id
    req_message->session_id = session_info->session_id;

    //Prepare the request message with the encrypted payload
    status = sgx_rijndael128GCM_encrypt(&session_info->active.AEK, (uint8_t*)inp_buff, data2encrypt_length,
                                        reinterpret_cast<uint8_t *>(&(req_message->message_aes_gcm_data.payload)),
                                        reinterpret_cast<uint8_t *>(&(req_message->message_aes_gcm_data.reserved)),
                                        sizeof(req_message->message_aes_gcm_data.reserved), plaintext, plaintext_length,
                                        &(req_message->message_aes_gcm_data.payload_tag));

    if(SGX_SUCCESS != status)
    {
        SAFE_FREE(req_message);
        return status;
    }

    //Allocate memory for the response payload to be copied
    *out_buff = (char*)malloc(max_out_buff_size);
    if(!*out_buff)
    {
        SAFE_FREE(req_message);
        return MALLOC_ERROR;
    }
    memset(*out_buff, 0, max_out_buff_size);

    //Allocate memory for the response message
    resp_message = (secure_message_t*)malloc(sizeof(secure_message_t)+ max_out_buff_size);
    if(!resp_message)
    {
        SAFE_FREE(req_message);
        return MALLOC_ERROR;
    }

    memset(resp_message, 0, sizeof(secure_message_t)+ max_out_buff_size);

    //Ocall to send the request to the Destination Enclave and get the response message back
    status = send_request_ocall(&retstatus, session_info->session_id, req_message,
                                (sizeof(secure_message_t)+ inp_buff_len), max_out_buff_size,
                                resp_message, (sizeof(secure_message_t)+ max_out_buff_size));
    if (status == SGX_SUCCESS)
    {
        if ((ATTESTATION_STATUS)retstatus != SUCCESS)
        {
            SAFE_FREE(req_message);
            SAFE_FREE(resp_message);
            return ((ATTESTATION_STATUS)retstatus);
        }
    }
    else
    {
        SAFE_FREE(req_message);
        SAFE_FREE(resp_message);
        return ATTESTATION_SE_ERROR;
    }

    max_resp_message_length = sizeof(secure_message_t)+ max_out_buff_size;

    if(sizeof(resp_message) > max_resp_message_length)
    {
        SAFE_FREE(req_message);
        SAFE_FREE(resp_message);
        return INVALID_PARAMETER_ERROR;
    }

    //Code to process the response message from the Destination Enclave

    decrypted_data_length = resp_message->message_aes_gcm_data.payload_size;
    plain_text_offset = decrypted_data_length;
    decrypted_data = (uint8_t*)malloc(decrypted_data_length);
    if(!decrypted_data)
    {
        SAFE_FREE(req_message);
        SAFE_FREE(resp_message);
        return MALLOC_ERROR;
    }
    memset(&l_tag, 0, 16);

    memset(decrypted_data, 0, decrypted_data_length);

    //Decrypt the response message payload
    status = sgx_rijndael128GCM_decrypt(&session_info->active.AEK, resp_message->message_aes_gcm_data.payload,
                                        decrypted_data_length, decrypted_data,
                                        reinterpret_cast<uint8_t *>(&(resp_message->message_aes_gcm_data.reserved)),
                                        sizeof(resp_message->message_aes_gcm_data.reserved), &(resp_message->message_aes_gcm_data.payload[plain_text_offset]), plaintext_length,
                                        &resp_message->message_aes_gcm_data.payload_tag);

    if(SGX_SUCCESS != status)
    {
        SAFE_FREE(req_message);
        SAFE_FREE(decrypted_data);
        SAFE_FREE(resp_message);
        return status;
    }

    // Verify if the nonce obtained in the response is equal to the session nonce + 1 (Prevents replay attacks)
    if(*(resp_message->message_aes_gcm_data.reserved) != (session_info->active.counter + 1 ))
    {
        SAFE_FREE(req_message);
        SAFE_FREE(resp_message);
        SAFE_FREE(decrypted_data);
        return INVALID_PARAMETER_ERROR;
    }

    //Update the value of the session nonce in the source enclave
    session_info->active.counter = session_info->active.counter + 1;

    memcpy(out_buff_len, &decrypted_data_length, sizeof(decrypted_data_length));
    memcpy(*out_buff, decrypted_data, decrypted_data_length);

    SAFE_FREE(decrypted_data);
    SAFE_FREE(req_message);
    SAFE_FREE(resp_message);
    return SUCCESS;
}

//Close a current session
ATTESTATION_STATUS close_session(dh_session_t *session_info)
{
    sgx_status_t status;
    uint32_t retstatus;

    if(!session_info)
    {
        return INVALID_PARAMETER_ERROR;
    }

    //Ocall to ask the destination enclave to end the session
    status = end_session_ocall(&retstatus, session_info->session_id);
    if (status == SGX_SUCCESS)
    {
        if ((ATTESTATION_STATUS)retstatus != SUCCESS)
            return ((ATTESTATION_STATUS)retstatus);
    }
    else
    {
        return ATTESTATION_SE_ERROR;
    }
    return SUCCESS;
}

//Returns a new sessionID for the source destination session
ATTESTATION_STATUS generate_session_id(uint32_t *session_id)
{
    ATTESTATION_STATUS status = SUCCESS;

    if(!session_id)
    {
        return INVALID_PARAMETER_ERROR;
    }
    //if the session structure is untintialized, set that as the next session ID
    for (int i = 0; i < MAX_SESSION_COUNT; i++)
    {
        if (g_session_id_tracker[i] == NULL)
        {
            *session_id = i;
            return status;
        }
    }

    status = NO_AVAILABLE_SESSION_ERROR;

    return status;

}
