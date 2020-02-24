#
# Copyright (C) 2011-2019 Intel Corporation. All rights reserved.
#
# Redistribution and use in source and binary forms, with or without
# modification, are permitted provided that the following conditions
# are met:
#
#   * Redistributions of source code must retain the above copyright
#     notice, this list of conditions and the following disclaimer.
#   * Redistributions in binary form must reproduce the above copyright
#     notice, this list of conditions and the following disclaimer in
#     the documentation and/or other materials provided with the
#     distribution.
#   * Neither the name of Intel Corporation nor the names of its
#     contributors may be used to endorse or promote products derived
#     from this software without specific prior written permission.
#
# THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
# "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
# LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
# A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
# OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
# SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
# LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
# DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
# THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
# (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
# OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
#
#

include ../buildenv.mk

ifneq ($(SGX_MODE), HW)
        Trts_Library_Name := sgx_trts_sim
        Service_Library_Name := sgx_tservice_sim
else
        Trts_Library_Name := sgx_trts
        Service_Library_Name := sgx_tservice
endif

ifneq ($(SGX_MODE), HW)
	Urts_Library_Name := sgx_urts_sim
else
	Urts_Library_Name := sgx_urts
endif

Crypto_Library_Name := sgx_tcrypto

ENCLAVE_NAME := libenclave_initiator.so
SIGNED_ENCLAVE_NAME := libenclave_initiator.signed.so

$(SIGNED_ENCLAVE_NAME) : $(ENCLAVE_NAME)
	@$(SGX_ENCLAVE_SIGNER) sign -key EnclaveInitiator_private.pem -enclave $(ENCLAVE_NAME) -out $@ -config EnclaveInitiator.config.xml
	@cp $(SIGNED_ENCLAVE_NAME) $(TOPDIR)/$(OUTDIR)/
	@echo "SIGN =>  $@"

# Enable the security flags
Enclave_Security_Link_Flags := -Wl,-z,relro,-z,now,-z,noexecstack

# To generate a proper enclave, it is recommended to follow below guideline to link the trusted libraries:
#    1. Link sgx_trts with the `--whole-archive' and `--no-whole-archive' options,
#       so that the whole content of trts is included in the enclave.
#    2. For other libraries, you just need to pull the required symbols.
#       Use `--start-group' and `--end-group' to link these libraries.
# Do NOT move the libraries linked with `--start-group' and `--end-group' within `--whole-archive' and `--no-whole-archive' options.
# Otherwise, you may get some undesirable errors.
ENCLAVE_LINK_FLAGS := $(Enclave_Security_Link_Flags) \
     -Wl,--no-undefined -nostdlib -nodefaultlibs -nostartfiles -L$(SGX_LIBRARY_PATH) \
             -Wl,--whole-archive  -l$(Trts_Library_Name) -Wl,--no-whole-archive \
             -Wl,--start-group -l$(Urts_Library_Name) -lsgx_tstdc -lsgx_tcxx -l$(Crypto_Library_Name) -l$(Service_Library_Name) -Wl,--end-group \
             -Wl,-Bstatic -Wl,-Bsymbolic -Wl,--no-undefined \
             -Wl,-pie,-eenclave_entry -Wl,--export-dynamic  \
             -Wl,--defsym,__ImageBase=0 -Wl,--gc-sections   \
             -Wl,--version-script=EnclaveInitiator.lds

SGX_COMMON_FLAGS += -Wall -Wextra -Winit-self -Wpointer-arith -Wreturn-type \
                    -Waddress -Wsequence-point -Wformat-security \
                    -Wmissing-include-dirs -Wfloat-equal -Wundef -Wshadow \
                    -Wcast-align -Wconversion -Wredundant-decls

SGX_COMMON_CFLAGS := $(SGX_COMMON_FLAGS) -Wjump-misses-init -Wstrict-prototypes -Wunsuffixed-float-constants
SGX_COMMON_CXXFLAGS := $(SGX_COMMON_FLAGS) -Wnon-virtual-dtor -std=c++11

Enclave_Include_Paths := -I$(SGX_SDK)/include -I$(SGX_SDK)/include/tlibc -I$(SGX_SDK)/include/libcxx -I../Include

Enclave_C_Flags := $(Enclave_Include_Paths) -nostdinc -fvisibility=hidden -fpie -ffunction-sections -fdata-sections
CC_BELOW_4_9 := $(shell expr "`$(CC) -dumpversion`" \< "4.9")
ifeq ($(CC_BELOW_4_9), 1)
        Enclave_C_Flags += -fstack-protector
else
        Enclave_C_Flags += -fstack-protector-strong
endif
Enclave_Cpp_Flags := $(Enclave_C_Flags) -nostdinc++

ifeq ($(LAv2), 1)
	Enclave_C_Flags += -DSGX_USE_LAv2_INITIATOR
	Enclave_Cpp_Flags += -DSGX_USE_LAv2_INITIATOR
endif

Enclave_Cpp_Files := $(wildcard *.cpp)
ENCLAVE_CPP_OBJECTS := $(Enclave_Cpp_Files:.cpp=.o)

.PHONY = all clean target

target = $(SIGNED_ENCLAVE_NAME)

all:
	@make target

clean:
	@rm -f  $(ENCLAVE_NAME) $(SIGNED_ENCLAVE_NAME) *.o *_t.c *_t.h *_u.c *_u.h

$(ENCLAVE_NAME):EnclaveInitiator_t.o $(ENCLAVE_CPP_OBJECTS)
	@$(CXX) $^ -o $@ $(ENCLAVE_LINK_FLAGS)
	@echo "LINK => $@"

######## Enclave Objects ########

EnclaveInitiator_t.h: $(SGX_EDGER8R) EnclaveInitiator.edl
	@$(SGX_EDGER8R) --trusted EnclaveInitiator.edl --search-path $(SGX_SDK)/include
	@echo "GEN  =>  $@"

EnclaveInitiator_t.c: EnclaveInitiator_t.h

EnclaveInitiator_t.o: EnclaveInitiator_t.c
	@$(CC) $(SGX_COMMON_CFLAGS) $(Enclave_C_Flags) -c $< -o $@
	@echo "CC   <=  $<"

%.o: %.cpp EnclaveInitiator_t.h
	@$(CXX) $(SGX_COMMON_CXXFLAGS) $(Enclave_Cpp_Flags) -c $< -o $@
	@echo "CXX  <=  $<"

