/*
 *
 * Copyright 2019 gRPC authors.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 *
 */

#ifndef SGX_RA_TLS_BACKEND_H
#define SGX_RA_TLS_BACKEND_H

#include <string>
#include <memory>
#include <sstream>



namespace grpc {
namespace sgx {

int verify_quote (uint8_t * quote_buffer, size_t quote_size);


int generate_quote(uint8_t *quote_buffer, unsigned char *hash, size_t hash_len);


uint32_t get_quote_size();

}
}
#endif  // SGX_RA_TLS_BACKEND_H
