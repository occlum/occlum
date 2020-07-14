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

# -------------------------------------------------------------------
#  Function : parent-dir
#  Arguments: 1: path
#  Returns  : Parent dir or path of $1, with final separator removed.
# -------------------------------------------------------------------
parent-dir = $(patsubst %/,%,$(dir $(1:%/=%)))

# ------------------------------------------------------------------
#  Macro    : my-dir
#  Returns  : the directory of the current Makefile
#  Usage    : $(my-dir)
# ------------------------------------------------------------------
my-dir = $(realpath $(call parent-dir,$(lastword $(MAKEFILE_LIST))))

ROOT_DIR := $(call my-dir)
ifneq ($(words $(subst :, ,$(ROOT_DIR))), 1)
  $(error main directory cannot contain spaces nor colons)
endif

# -------------------------------------------------------------------
# User needs to install SGX SDK installer before compiling this project.
# -------------------------------------------------------------------
ifeq ($(SGX_SDK),)
$(warning "import SGX SDK environment variable")
export SGX_SDK=/opt/intel/sgxsdk/
endif

SGX_SDK_PATH ?= $(SGX_SDK)

#-------------------------------------------------------------------
# This is the output folder.
#-------------------------------------------------------------------
BUILD_DIR := build
TOPDIR = $(ROOT_DIR)
OUTDIR := $(BUILD_DIR)

CP = cp
CC ?= gcc
CXX ?= g++

# turn on cet
CC_GREAT_EQUAL_8 := $(shell expr "`$(CC) -dumpversion`" \>= "8")
ifeq ($(CC_GREAT_EQUAL_8), 1)
    COMMON_FLAGS += -fcf-protection
endif

# ------------------------------------------------------------------
#  Define common variables
# ------------------------------------------------------------------
SGX_MODE ?= HW
SGX_ARCH ?= x64
SGX_DEBUG ?= 1

#-------------------------------------------------------------------
# Define common compile flags used for GCC and G++
#-------------------------------------------------------------------
COMMON_FLAGS = -ffunction-sections -fdata-sections

COMMON_FLAGS += -Wall -Wextra -Wchar-subscripts -Wno-coverage-mismatch -Winit-self \
		-Wpointer-arith -Wreturn-type -Waddress -Wsequence-point -Wformat-security \
		-Wmissing-include-dirs -Wfloat-equal -Wundef -Wshadow \
		-Wcast-align -Wconversion -Wredundant-decls

ifeq ($(SGX_DEBUG), 1)
ifeq ($(SGX_PRERELEASE), 1)
$(error Cannot set SGX_DEBUG and SGX_PRERELEASE at the same time!!)
endif
endif

ifeq ($(SGX_DEBUG), 1)
	COMMON_FLAGS += -ggdb -DDEBUG
	COMMON_FLAGS += -DDEBUG_LEVEL=TRACE_DEBUG
else
	COMMON_FLAGS += -o2 -UDEBUG
endif

CFLAGS = $(COMMON_FLAGS)
CXXFLAGS = $(COMMON_FLAGS)

# additional warnings flags for C
CFLAGS += -Wjump-misses-init -Wstrict-prototypes -Wunsuffixed-float-constants

# additional warnings flags for C++
CXXFLAGS += -Wnon-virtual-dtor -std=c++11

# ----------------------------------------------------------------
#  Define common link options
# ----------------------------------------------------------------
COMMON_LDFLAGS := -Wl,-z,relro,-z,now,-z,noexecstack

# Compiler and linker options for an Enclave
#
# We are using '--export-dynamic' so that `g_global_data_sim' etc.
# will be exported to dynamic symbol table.
#
# When `pie' is enabled, the linker (both BFD and Gold) under Ubuntu 14.04
# will hide all symbols from dynamic symbol table even if they are marked
# as `global' in the LD version script.
ENCLAVE_CFLAGS   = -ffreestanding -nostdinc -fvisibility=hidden -fpie
ifeq ($(CC_GREAT_EQUAL_8), 1)
    ENCLAVE_CFLAGS += -fcf-protection
endif
ENCLAVE_CXXFLAGS = $(ENCLAVE_CFLAGS) -nostdinc++
ENCLAVE_LDFLAGS  = $(COMMON_LDFLAGS)

RM = rm -f

ifeq ($(shell getconf LONG_BIT), 32)
        SGX_ARCH := x86
else ifeq ($(findstring -m32, $(CXXFLAGS)), -m32)
        SGX_ARCH := x86
endif

ifeq ($(SGX_ARCH), x86)
        SGX_COMMON_FLAGS := -m32
        SGX_LIBRARY_PATH := $(SGX_SDK)/lib
        SGX_ENCLAVE_SIGNER := $(SGX_SDK)/bin/x86/sgx_sign
        SGX_EDGER8R := $(SGX_SDK)/bin/x86/sgx_edger8r
else
        SGX_COMMON_FLAGS := -m64
        SGX_LIBRARY_PATH := $(SGX_SDK)/lib64
        SGX_ENCLAVE_SIGNER := $(SGX_SDK)/bin/x64/sgx_sign
        SGX_EDGER8R := $(SGX_SDK)/bin/x64/sgx_edger8r
endif

SGX_COMMON_FLAGS += $(COMMON_FLAGS)
