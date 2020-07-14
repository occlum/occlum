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

TARGET = appinitiator

RM = rm -f

OCCLUM_PREFIX ?= /opt/occlum

CFLAGS := -Wall \
        -I$(SGX_SDK)/include \
        -I$(OCCLUM_PREFIX)/include \
		-I../Include
CXXFLAGS := -Wall \
        -I$(SGX_SDK)/include \
        -I$(OCCLUM_PREFIX)/include \
		-I../Include

ifneq ($(SGX_MODE), HW)
	URTS_LIB_NAME := sgx_urts_sim
	UAE_SERVICE_LIB := sgx_uae_service_sim
	OCCLUM_PAL_LIB := occlum-pal_sim
else
	URTS_LIB_NAME := sgx_urts
	UAE_SERVICE_LIB := sgx_uae_service
	OCCLUM_PAL_LIB := occlum-pal
endif

MAJOR_VER_NUM = $(shell grep '\#define OCCLUM_MAJOR_VERSION' $(OCCLUM_PREFIX)/include/occlum_version.h |  awk '{print $$3}')
MINOR_VER_NUM = $(shell grep '\#define OCCLUM_MINOR_VERSION' $(OCCLUM_PREFIX)/include/occlum_version.h |  awk '{print $$3}')
PATCH_VER_NUM = $(shell grep '\#define OCCLUM_PATCH_VERSION' $(OCCLUM_PREFIX)/include/occlum_version.h |  awk '{print $$3}')
VERSION_NUM = $(MAJOR_VER_NUM).$(MINOR_VER_NUM).$(PATCH_VER_NUM)

INC:=-I$(SGX_SDK)/include -I../Include
LIB := -l$(URTS_LIB_NAME) -l$(UAE_SERVICE_LIB) -L$(SGX_SDK)/lib64 -lpthread
CXXFLAGS += $(INC) $(LIB)
CFLAGS += $(INC) $(LIB)

ifeq ($(SGX_DEBUG), 1)
        CXXFLAGS += -DDEBUG -UNDEBUG -UEDEBUG
        CFLAGS += -DDEBUG -UNDEBUG -UEDEBUG
else ifeq ($(SGX_PRERELEASE), 1)
        CXXFLAGS += -DEDEBUG -DNDEBUG -UDEBUG
        CFLAGS += -DEDEBUG -DNDEBUG -UDEBUG
else
        CXXFLAGS += -DNDEBUG -UEDEBUG -UDEBUG
        CFLAGS += -DNDEBUG -UEDEBUG -UDEBUG
endif

LINK_FLAGS := $(CXXFLAGS) -lpthread \
		-L$(SGX_SDK)/lib64 -lsgx_uprotected_fs \
		-L$(TOPDIR)/$(OUTDIR) -locclum-pal

SRC_CPP=$(wildcard *.cpp)

SRC_OBJ += $(SRC_CPP:.cpp=.o)
SRC_OBJ += $(SRC_C:.c=.o)


.PHONY = all clean

all: $(TARGET)

SGX_COMMON_CFLAGS := -I$(SGX_SDK)/include

UntrustedEnclaveMessageExchange.o: UntrustedEnclaveMessageExchange.cpp
	@$(CXX) $(CXXFLAGS) -c $< -o $@
	@echo "CC   <=  $<"

App.o: App.cpp
	@$(CXX) $(CXXFLAGS) -c $< -o $@
	@echo "CXX   <=  $<"

fifo.o: fifo.cpp
	@$(CXX) $(CXXFLAGS) -c $< -o $@
	@echo "CXX   <=  $<"

EnclaveInitiator_u.o: EnclaveInitiator_u.c
	@$(CC) $(CFLAGS) -c $< -o $@
	@echo "CC   <=  $<"

EnclaveInitiator_u.c:
	@$(SGX_EDGER8R) --untrusted ../EnclaveInitiator/EnclaveInitiator.edl --search-path $(SGX_SDK)/include

libocclum-pal.so:
	@mkdir -p $(TOPDIR)/$(OUTDIR)
	@cd $(TOPDIR)/$(OUTDIR) && ln -sf $(OCCLUM_PREFIX)/build/lib/lib$(OCCLUM_PAL_LIB).so.$(VERSION_NUM) libocclum-pal.so.$(MAJOR_VER_NUM) && \
		ln -sf libocclum-pal.so.$(MAJOR_VER_NUM) libocclum-pal.so

$(TARGET): EnclaveInitiator_u.o $(SRC_OBJ) libocclum-pal.so
	@$(CXX) EnclaveInitiator_u.o $(SRC_OBJ) $(LINK_FLAGS) -o $@
	@echo "GEN  =>  $@"
	@mv $@ $(TOPDIR)/$(OUTDIR)/

clean:
	@$(RM) $(TARGET) *.o *_u.c *_u.h libocclum-pal.so.$(MAJOR_VER_NUM) libocclum-pal.so
