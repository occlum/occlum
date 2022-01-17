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

#include "sgx_ra_tls_utils.h"

#include <cstring>
#include <iostream>
#include <fstream>
#include <sstream>

namespace grpc {
namespace sgx {

void check_free(void* ptr) {
    if (ptr) {
        free(ptr);
        ptr = nullptr;
    };
}

bool hex_to_byte(const char *src, char *dst, size_t dst_size) {
    if (std::strlen(src) < dst_size*2) {
        return false;
    } else {
        for (auto i = 0; i < dst_size; i++) {
            if (!isxdigit(src[i*2]) || !isxdigit(src[i*2+1])) {
                return false;
            } else {
                sscanf(src+i*2, "%02hhx", dst+i);
            }
        }
        return true;
    }
};

void byte_to_hex(const char *src, char *dst, size_t src_size) {
    for (auto i = 0; i < src_size; i++) {
        sprintf(dst+i*2, "%02hhx", src[i]);
    }
};

std::string byte_to_hex(const char *src, size_t src_size) {
    char dst[src_size*2];
    memset(dst, 0, sizeof(dst));
    byte_to_hex(src, dst, src_size);
    return std::string(dst);
};

library_engine::library_engine() : handle(nullptr), error(nullptr) {};

library_engine::library_engine(const char* file, int mode) : handle(nullptr), error(nullptr) {
    this->open(file, mode);
}

library_engine::~library_engine() {
    this->close();
}

void library_engine::open(const char* file, int mode) {
    this->close();
    handle = dlopen(file, mode);
    error = dlerror();
    if (error != nullptr || handle == nullptr) {
        throw std::runtime_error("dlopen " + std::string(file) + " error, " + std::string(error));
    }
}

void library_engine::close() {
    if (handle) {
        dlclose(handle);
    }
    handle = nullptr;
    error = nullptr;
}

void* library_engine::get_func(const char* name) {
  auto func = dlsym(handle, name);
  error = dlerror();
  if (error != nullptr || func == nullptr) {
    throw std::runtime_error("dlsym " + std::string(name) + " error, " + std::string(error));
    return nullptr;
  } else {
    return func;
  }
}

void* library_engine::get_handle() {
    return handle;
}

json_engine::json_engine() : handle(nullptr) {};

json_engine::json_engine(const char* file) : handle(nullptr){
    this->open(file);
}

json_engine::~json_engine() {
    this->close();
}

bool json_engine::open(const char* file) {
    if (!file) {
        grpc_printf("wrong json file path\n");
        return false;
    }

    this->close();

    auto file_ptr = fopen(file, "r");
    fseek(file_ptr, 0, SEEK_END);
    auto length = ftell(file_ptr);
    fseek(file_ptr, 0, SEEK_SET);
    auto buffer = malloc(length);
    fread(buffer, 1, length, file_ptr);
    fclose(file_ptr);

    this->handle = cJSON_Parse((const char *)buffer);

    check_free(buffer);

    if (this->handle) {
        return true;
    } else {
        grpc_printf("cjson open %s error: %s", file, cJSON_GetErrorPtr());
        return false;
    }
}

void json_engine::close() {
    if (this->handle) {
        cJSON_Delete(this->handle);
        this->handle = nullptr;
    }
}

cJSON* json_engine::get_handle() {
    return this->handle;
}

cJSON* json_engine::get_item(cJSON* obj, const char* item) {
    return cJSON_GetObjectItem(obj, item);
};

char* json_engine::print_item(cJSON* obj) {
    return cJSON_Print(obj);
};

bool json_engine::compare_item(cJSON* obj, const char* item) {
    auto obj_item = this->print_item(obj);
    return strncmp(obj_item+1, item, std::min(strlen(item), strlen(obj_item)-2)) == 0;
};

} // namespace sgx
} // namespace grpc
