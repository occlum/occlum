/*
 *
 * Copyright (c) 2022 Intel Corporation
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
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

#include "../grpc_ratls_client.h"


int main(int argc, char** argv) {
    // Parse arguments
    if (argc < 4) {
        printf("[ERROR] Three arguments must be provided\n\n");
        printf("Usage: client <grpc-server addr> <request_name> <secret_file_to_be_saved>\n");
        return -1;
    }

    grpc_ratls_get_secret(
        argv[1],
        "dynamic_config.json",
        argv[2],
        argv[3]
    );

    return 0;
}
