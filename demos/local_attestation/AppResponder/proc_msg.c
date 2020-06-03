#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sched.h>
#include "sgx_tseal.h"
#include "session.h"
#include "proc_msg.h"

#define SAFE_FREE(ptr) {if (NULL != (ptr)) {free(ptr); (ptr) = NULL;}}
#define MESSAGE_EXCHANGE 0x0

int proc(FIFO_MSG *message);
extern int m_shutdown;

typedef struct _secure_message_t {
    uint32_t session_id; //Session ID identifyting the session to which the message belongs
    sgx_aes_gcm_data_t message_aes_gcm_data;
} secure_message_t;

typedef struct _ms_in_msg_exchange_t {
    uint32_t msg_type; //Type of Call E2E or general message exchange
    uint32_t target_fn_id; //Function Id to be called in Destination. Is valid only when msg_type=ENCLAVE_TO_ENCLAVE_CALL
    uint32_t inparam_buff_len; //Length of the serialized input parameters
    char inparam_buff[1]; //Serialized input parameters
} ms_in_msg_exchange_t;

//Format of the return value and output function parameter structure
typedef struct _ms_out_msg_exchange_t {
    uint32_t retval_len; //Length of the return value
    uint32_t ret_outparam_buff_len; //Length of the serialized return value and output parameters
    char ret_outparam_buff[1]; //Serialized return value and output parameters
} ms_out_msg_exchange_t;

/* Function Description:
 *  This function responds to initiator enclave's connection request by generating and sending back ECDH message 1
 * Parameter Description:
 *  [input] clientfd: this is client's connection id. After generating ECDH message 1, server would send back response through this connection id.
 * */
int generate_and_send_session_msg1_resp(int clientfd) {
    int retcode = 0;
    uint32_t status = 0;
    sgx_status_t ret = SGX_SUCCESS;
    SESSION_MSG1_RESP msg1resp;
    FIFO_MSG *fifo_resp = NULL;
    size_t respmsgsize;

    memset(&msg1resp, 0, sizeof(SESSION_MSG1_RESP));

    // call responder enclave to generate ECDH message 1
    ret = session_request(&msg1resp.dh_msg1, &msg1resp.sessionid);
    if (ret != SGX_SUCCESS) {
        printf("failed to do ECALL session_request.\n");
        return -1;
    }

    respmsgsize = sizeof(FIFO_MSG) + sizeof(SESSION_MSG1_RESP);
    fifo_resp = (FIFO_MSG *)malloc(respmsgsize);
    if (!fifo_resp) {
        printf("memory allocation failure.\n");
        return -1;
    }
    memset(fifo_resp, 0, respmsgsize);

    fifo_resp->header.type = FIFO_DH_RESP_MSG1;
    fifo_resp->header.size = sizeof(SESSION_MSG1_RESP);

    memcpy(fifo_resp->msgbuf, &msg1resp, sizeof(SESSION_MSG1_RESP));

    //send message 1 to client
    if (send(clientfd, (char *)(fifo_resp), (int)(respmsgsize), 0) == -1) {
        printf("fail to send msg1 response.\n");
        retcode = -1;
    }
    free(fifo_resp);
    return retcode;
}
/* Function Description:
 *  This function process ECDH message 2 received from client and send message 3 to client
 * Parameter Description:
 *  [input] clientfd: this is client's connection id
 *  [input] msg2: this contains ECDH message 2 received from client
 * */
int process_exchange_report(int clientfd, SESSION_MSG2 *msg2) {
    uint32_t status = 0;
    sgx_status_t ret = SGX_SUCCESS;
    FIFO_MSG *response;
    SESSION_MSG3 *msg3;
    size_t msgsize;

    if (!msg2) {
        return -1;
    }

    msgsize = sizeof(FIFO_MSG_HEADER) + sizeof(SESSION_MSG3);
    response = (FIFO_MSG *)malloc(msgsize);
    if (!response) {
        printf("memory allocation failure\n");
        return -1;
    }
    memset(response, 0, msgsize);

    response->header.type = FIFO_DH_MSG3;
    response->header.size = sizeof(SESSION_MSG3);

    msg3 = (SESSION_MSG3 *)response->msgbuf;
    msg3->sessionid = msg2->sessionid;

    // call responder enclave to process ECDH message 2 and generate message 3
    ret = exchange_report(&msg2->dh_msg2, &msg3->dh_msg3, msg2->sessionid);
    if (ret != SGX_SUCCESS) {
        printf("Enclave Response_exchange_report failure.\n");
        free(response);
        return -1;
    }

    // send ECDH message 3 to client
    if (send(clientfd, (char *)(response), (int)(msgsize), 0) == -1) {
        printf("server_send() failure.\n");
        free(response);
        return -1;
    }

    free(response);

    return 0;
}
uint32_t get_message_exchange_response(uint32_t inp_secret_data) {
    uint32_t secret_response;
    printf("secret=0x%x\n", inp_secret_data);
    //User should use more complex encryption method to protect their secret, below is just a simple example
    secret_response = inp_secret_data & 0x11111111;

    return secret_response;

}
int  umarshal_message_exchange_request(uint32_t *inp_secret_data,
                                       ms_in_msg_exchange_t *ms) {
    char *buff;
    size_t len;
    if (!inp_secret_data || !ms) {
        return -1;
    }
    buff = ms->inparam_buff;
    len = ms->inparam_buff_len;
    if (len != sizeof(uint32_t)) {
        return -1;
    }

    memcpy(inp_secret_data, buff, sizeof(uint32_t));

    return 0;
}
int  marshal_message_exchange_response(char **resp_buffer, size_t *resp_length,
                                       uint32_t secret_response) {
    ms_out_msg_exchange_t *ms;
    size_t secret_response_len, ms_len;
    size_t retval_len, ret_param_len;
    if (!resp_length) {
        return -1;
    }
    secret_response_len = sizeof(secret_response);
    retval_len = secret_response_len;
    ret_param_len = secret_response_len;
    ms_len = sizeof(ms_out_msg_exchange_t) + ret_param_len;
    ms = (ms_out_msg_exchange_t *)malloc(ms_len);
    if (!ms) {
        return -1;
    }

    ms->retval_len = (uint32_t)retval_len;
    ms->ret_outparam_buff_len = (uint32_t)ret_param_len;
    memcpy(&ms->ret_outparam_buff, &secret_response, secret_response_len);
    *resp_buffer = (char *)ms;
    *resp_length = ms_len;
    return 0;
}
int  message_exchange_response_generator(char *decrypted_data,
        char **resp_buffer,
        size_t *resp_length) {
    ms_in_msg_exchange_t *ms;
    uint32_t inp_secret_data;
    uint32_t out_secret_data;

    if (!decrypted_data || !resp_length) {
        return -1;
    }

    ms = (ms_in_msg_exchange_t *)decrypted_data;

    if (umarshal_message_exchange_request(&inp_secret_data, ms) < 0) {
        return -1;
    }

    out_secret_data = get_message_exchange_response(inp_secret_data);

    if (marshal_message_exchange_response(resp_buffer, resp_length, out_secret_data)  < 0) {
        return -1;
    }

    return 0;
}
int generate_response(secure_message_t *req_message,
                      size_t req_message_size,
                      size_t max_payload_size,
                      secure_message_t *resp_message,
                      size_t resp_message_size,
                      uint32_t session_id) {
#define TAG_SIZE        16
    const uint8_t *plaintext;
    uint32_t plaintext_length;
    uint8_t *decrypted_data;
    uint32_t decrypted_data_length;
    uint32_t plain_text_offset;
    ms_in_msg_exchange_t *ms;
    size_t resp_data_length;
    size_t resp_message_calc_size;
    char *resp_data;
    uint8_t l_tag[TAG_SIZE];
    size_t header_size, expected_payload_size;
    dh_session_t *session_info;
    secure_message_t *temp_resp_message;
    uint32_t ret;
    sgx_status_t status;

    plaintext = (const uint8_t *)(" ");
    plaintext_length = 0;

    if (!req_message || !resp_message) {
        return -1;
    }

    //Get the session information from the map corresponding to the source enclave id
    session_info = get_session_info(session_id);
    if (session_info == NULL) { return -1; }
    //Set the decrypted data length to the payload size obtained from the message
    decrypted_data_length = req_message->message_aes_gcm_data.payload_size;

    header_size = sizeof(secure_message_t);
    expected_payload_size = req_message_size - header_size;

    //Verify the size of the payload
    if (expected_payload_size != decrypted_data_length) {
        return -1;
    }

    memset(&l_tag, 0, 16);
    plain_text_offset = decrypted_data_length;
    decrypted_data = (uint8_t *)malloc(decrypted_data_length);
    if (!decrypted_data) {
        return -1;
    }

    memset(decrypted_data, 0, decrypted_data_length);
    status = sgx_rijndael128GCM_decrypt(&session_info->active.AEK,
                                        req_message->message_aes_gcm_data.payload,
                                        decrypted_data_length, decrypted_data,
                                        (uint8_t *)(&(req_message->message_aes_gcm_data.reserved)),
                                        sizeof(req_message->message_aes_gcm_data.reserved),
                                        &(req_message->message_aes_gcm_data.payload[plain_text_offset]), plaintext_length,
                                        &req_message->message_aes_gcm_data.payload_tag);

    if (SGX_SUCCESS != status) {
        SAFE_FREE(decrypted_data);
        return -1;
    }

    //Casting the decrypted data to the marshaling structure type to obtain type of request (generic message exchange/enclave to enclave call)
    ms = (ms_in_msg_exchange_t *)decrypted_data;

    // Verify if the nonce obtained in the request is equal to the session nonce
    if ((uint32_t) * (req_message->message_aes_gcm_data.reserved) !=
            session_info->active.counter ||
            *(req_message->message_aes_gcm_data.reserved) > ((2 ^ 32) - 2)) {
        SAFE_FREE(decrypted_data);
        return -1;
    }

    if (ms->msg_type == MESSAGE_EXCHANGE) {
        //Call the generic secret response generator for message exchange
        ret = message_exchange_response_generator((char *)decrypted_data, &resp_data,
                &resp_data_length);
        if (ret != 0) {
            SAFE_FREE(decrypted_data);
            SAFE_FREE(resp_data);
            return -1;
        }
    } else {
        SAFE_FREE(decrypted_data);
        return -1;
    }


    if (resp_data_length > max_payload_size) {
        SAFE_FREE(resp_data);
        SAFE_FREE(decrypted_data);
        return -1;
    }

    resp_message_calc_size = sizeof(secure_message_t) + resp_data_length;

    if (resp_message_calc_size > resp_message_size) {
        SAFE_FREE(resp_data);
        SAFE_FREE(decrypted_data);
        return -1;
    }

    //Code to build the response back to the Source Enclave
    temp_resp_message = (secure_message_t *)malloc(resp_message_calc_size);
    if (!temp_resp_message) {
        SAFE_FREE(resp_data);
        SAFE_FREE(decrypted_data);
        return -1;
    }
    memset(temp_resp_message, 0, sizeof(secure_message_t) + resp_data_length);
    const uint32_t data2encrypt_length = (uint32_t)resp_data_length;
    temp_resp_message->session_id = session_info->session_id;
    temp_resp_message->message_aes_gcm_data.payload_size = data2encrypt_length;

    //Increment the Session Nonce (Replay Protection)
    session_info->active.counter = session_info->active.counter + 1;

    //Set the response nonce as the session nonce
    memcpy(&temp_resp_message->message_aes_gcm_data.reserved, &session_info->active.counter,
           sizeof(session_info->active.counter));

    //Prepare the response message with the encrypted payload
    status = sgx_rijndael128GCM_encrypt(&session_info->active.AEK, (uint8_t *)resp_data,
                                        data2encrypt_length,
                                        (uint8_t *)(&(temp_resp_message->message_aes_gcm_data.payload)),
                                        (uint8_t *)(&(temp_resp_message->message_aes_gcm_data.reserved)),
                                        sizeof(temp_resp_message->message_aes_gcm_data.reserved), plaintext, plaintext_length,
                                        &(temp_resp_message->message_aes_gcm_data.payload_tag));

    if (SGX_SUCCESS != status) {
        SAFE_FREE(resp_data);
        SAFE_FREE(decrypted_data);
        SAFE_FREE(temp_resp_message);
        return -1;
    }

    memset(resp_message, 0, sizeof(secure_message_t) + resp_data_length);
    memcpy(resp_message, temp_resp_message, sizeof(secure_message_t) + resp_data_length);

    SAFE_FREE(decrypted_data);
    SAFE_FREE(resp_data);
    SAFE_FREE(temp_resp_message);

    return 0;
}

int process_msg_transfer(int clientfd, FIFO_MSGBODY_REQ *req_msg) {
    uint32_t status = 0;
    sgx_status_t ret = SGX_SUCCESS;
    secure_message_t *resp_message = NULL;
    FIFO_MSG *fifo_resp = NULL;
    size_t resp_message_size;

    if (!req_msg) {
        printf("invalid parameter.\n");
        return -1;
    }

    resp_message_size = sizeof(secure_message_t) + req_msg->max_payload_size;
    //Allocate memory for the response message
    resp_message = (secure_message_t *)malloc(resp_message_size);
    if (!resp_message) {
        printf("memory allocation failure.\n");
        return -1;
    }
    memset(resp_message, 0, resp_message_size);

    ret = generate_response( (secure_message_t *)req_msg->buf, req_msg->size,
                             req_msg->max_payload_size, resp_message, resp_message_size, req_msg->session_id);
    if (ret < 0) {
        printf("EnclaveResponder_generate_response error.\n");
        free(resp_message);
        return -1;
    }

    fifo_resp = (FIFO_MSG *)malloc(sizeof(FIFO_MSG) + resp_message_size);
    if (!fifo_resp) {
        printf("memory allocation failure.\n");
        free(resp_message);
        return -1;
    }
    memset(fifo_resp, 0, sizeof(FIFO_MSG) + resp_message_size);

    fifo_resp->header.type = FIFO_DH_MSG_RESP;
    fifo_resp->header.size = resp_message_size;
    memcpy(fifo_resp->msgbuf, resp_message, resp_message_size);

    free(resp_message);

    if (send(clientfd, (char *)(fifo_resp), sizeof(FIFO_MSG) + (int)(resp_message_size),
             0) == -1) {
        printf("server_send() failure.\n");
        free(fifo_resp);
        return -1;
    }
    free(fifo_resp);

    return 0;
}
int process_close_req(int clientfd, SESSION_CLOSE_REQ *close_req) {
    uint32_t status = 0;
    sgx_status_t ret = SGX_SUCCESS;
    FIFO_MSG close_ack;

    if (!close_req) {
        return -1;
    }

    // call responder enclave to close this session
    ret = end_session( close_req->session_id);
    if (ret != SGX_SUCCESS) {
        return -1;
    }

    // send back response
    close_ack.header.type = FIFO_DH_CLOSE_RESP;
    close_ack.header.size = 0;

    if (send(clientfd, (char *)(&close_ack), sizeof(FIFO_MSG), 0) == -1) {
        printf("server_send() failure.\n");
        return -1;
    }

    return 0;
}
int proc (FIFO_MSG *message) {
    if (message == NULL) { return 0; }
    switch (message->header.type) {
        case FIFO_DH_REQ_MSG1: {
            // process ECDH session connection request
            int clientfd = message->header.sockfd;

            if (generate_and_send_session_msg1_resp(clientfd) != 0) {
                printf("failed to generate and send session msg1 resp.\n");
                break;
            } else { printf("generate and send session msg1 resp.\n"); }

        }
        break;

        case FIFO_DH_MSG2: {
            // process ECDH message 2
            int clientfd = message->header.sockfd;
            SESSION_MSG2 *msg2 = NULL;
            msg2 = (SESSION_MSG2 *)message->msgbuf;

            if (process_exchange_report(clientfd, msg2) != 0) {
                printf("failed to process exchange_report request.\n");
                break;
            } else { printf(" process exchange_report request.\n"); }
        }
        break;
        case FIFO_DH_MSG_REQ: {
            // process message transfer request
            int clientfd = message->header.sockfd;
            FIFO_MSGBODY_REQ *msg = NULL;

            msg = (FIFO_MSGBODY_REQ *)message->msgbuf;

            if (process_msg_transfer(clientfd, msg) != 0) {
                printf("failed to process message transfer request.\n");
                break;
            }
        }
        break;

        case FIFO_DH_CLOSE_REQ: {
            // process message close request
            int clientfd = message->header.sockfd;

            SESSION_CLOSE_REQ *closereq = NULL;
            closereq = (SESSION_CLOSE_REQ *)message->msgbuf;
            process_close_req(clientfd, closereq);
            printf("process close_requestt request.\n");
            m_shutdown = 1;
        }
        break;
        default: {
            printf("Unknown message.\n");
        }
        break;
    }
    free(message);
}
