MAIN_MAKEFILE := $(firstword $(MAKEFILE_LIST))
INCLUDE_MAKEFILE := $(lastword $(MAKEFILE_LIST))
CUR_DIR := $(shell dirname $(realpath $(MAIN_MAKEFILE)))
PROJECT_DIR := $(realpath $(CUR_DIR)/../../)

SHELL := /bin/bash

SGX_SDK ?= /opt/intel/sgxsdk
SGX_MODE ?= HW
SGX_ARCH ?= x64

MAJOR_VER_NUM = $(shell grep '\#define OCCLUM_MAJOR_VERSION' $(PROJECT_DIR)/src/pal/include/occlum_version.h |  awk '{print $$3}')
MINOR_VER_NUM = $(shell grep '\#define OCCLUM_MINOR_VERSION' $(PROJECT_DIR)/src/pal/include/occlum_version.h |  awk '{print $$3}')
PATCH_VER_NUM = $(shell grep '\#define OCCLUM_PATCH_VERSION' $(PROJECT_DIR)/src/pal/include/occlum_version.h |  awk '{print $$3}')
VERSION_NUM = $(MAJOR_VER_NUM).$(MINOR_VER_NUM).$(PATCH_VER_NUM)

C_FORMATTER := $(PROJECT_DIR)/tools/c_formatter
# Use echo program instead of built-in echo command in shell. This ensures
# that echo can recognize escaped sequences (with -e argument) regardless of
# the specific shell (e.g., bash, zash, etc.)
ECHO := /bin/echo -e
# Shell escaped sequences for colorful output
CYAN := \033[1;36m
GREEN := \033[1;32m
RED := \033[1;31m
NO_COLOR := \033[0m

# Save code and object file generated during building src
OBJ_DIR := $(PROJECT_DIR)/build/internal/src
ifneq ($(SGX_MODE), HW)
	SRC_OBJ := src_sim
else
	SRC_OBJ := src
endif

BUILD_DIR := $(PROJECT_DIR)/build

# If OCCLUM_RELEASE_BUILD equals to 1, y, or yes, then build in release mode
OCCLUM_RELEASE_BUILD ?= 0
ifeq ($(OCCLUM_RELEASE_BUILD), yes)
	OCCLUM_RELEASE_BUILD := 1
else ifeq ($(OCCLUM_RELEASE_BUILD), y)
	OCCLUM_RELEASE_BUILD := 1
endif

ifeq ($(shell getconf LONG_BIT), 32)
	SGX_ARCH := x86
else ifeq ($(findstring -m32, $(CXXFLAGS)), -m32)
	SGX_ARCH := x86
endif

SGX_COMMON_CFLAGS := -Wall -std=gnu11

ifeq ($(SGX_ARCH), x86)
	SGX_COMMON_CFLAGS += -m32
	SGX_LIBRARY_PATH := $(SGX_SDK)/lib
	SGX_ENCLAVE_SIGNER := $(SGX_SDK)/bin/x86/sgx_sign
	SGX_EDGER8R := $(SGX_SDK)/bin/x86/sgx_edger8r
else
	SGX_COMMON_CFLAGS += -m64
	SGX_LIBRARY_PATH := $(SGX_SDK)/lib64
	SGX_ENCLAVE_SIGNER := $(SGX_SDK)/bin/x64/sgx_sign
	SGX_EDGER8R := $(SGX_SDK)/bin/x64/sgx_edger8r
endif

ifeq ($(OCCLUM_RELEASE_BUILD), 1)
	SGX_COMMON_CFLAGS += -O2
else
	SGX_COMMON_CFLAGS += -O0 -g
endif

RUST_SGX_SDK_DIR := $(PROJECT_DIR)/deps/rust-sgx-sdk

ifneq ($(SGX_MODE), HW)
	SGX_COMMON_CFLAGS += -D SGX_MODE_SIM
else
	SGX_COMMON_CFLAGS += -D SGX_MODE_HW
endif

ifneq ($(SGX_MODE), HW)
	Trts_Library_Name := sgx_trts_sim
	Service_Library_Name := sgx_tservice_sim
else
	Trts_Library_Name := sgx_trts
	Service_Library_Name := sgx_tservice
endif
Crypto_Library_Name := sgx_tcrypto
KeyExchange_Library_Name := sgx_tkey_exchange
ProtectedFs_Library_Name := sgx_tprotected_fs

#
# Export flags used to compile or link untrusted modules
#
SGX_CFLAGS_U := $(SGX_COMMON_CFLAGS) -fPIC -Wno-attributes \
	-I$(RUST_SGX_SDK_DIR)/edl -I$(SGX_SDK)/include
SGX_CXXFLAGS_U := $(SGX_CFLAGS_U) -std=c++11

ifneq ($(SGX_MODE), HW)
	SGX_LFLAGS_U := $(SGX_COMMON_CFLAGS) -lpthread -L$(SGX_LIBRARY_PATH) -Wl,-Bstatic -lsgx_urts_sim -Wl,-Bdynamic -lsgx_uae_service_sim
else
	SGX_LFLAGS_U := $(SGX_COMMON_CFLAGS) -lpthread -L$(SGX_LIBRARY_PATH) -Wl,-Bstatic -lsgx_urts -Wl,-Bdynamic -lsgx_uae_service -lsgx_enclave_common
endif

#
# Export flags used to compile or link untrusted modules
#
SGX_CFLAGS_T := $(SGX_COMMON_CFLAGS) -nostdinc -fvisibility=hidden -fpie -fstack-protector \
	-I$(RUST_SGX_SDK_DIR)/common/inc -I$(RUST_SGX_SDK_DIR)/edl -I$(SGX_SDK)/include -I$(SGX_SDK)/include/tlibc
SGX_CXXFLAGS_T := $(SGX_CFLAGS_T) -std=c++11 -nostdinc++ -I$(SGX_SDK)/include/libcxx

# Before use this linker flag, the user should define $(_Other_Enclave_Libs),
# and $(_Other_Link_Flags)
#
# Linker arguments:
# --no-undefined: Report unresolved symbols.
# --whole-archive <libs> --no-whole-archive: Force including all object files 
#  in the libraries <libs>. Normally, only required object files are included
#  by the linker.
# --start-group <libs> --end-group: Link libraries <libs>, resolve any circular 
#  dependencies between them.
# -Bstatic: Do not link against shared libraries.
# -Bsymbolic: Bind references to global symbols to the definition within this 
#  shared library.
# -pie: Create a position independent executable
# --defsym=<symbol>=<value>: Define a symbol with the specified value
# --gc-sections: Enable link-time garbage collection, i.e., eliminating unused 
#  sections. See -ffunction-sections and -fdata-section of GCC.
#
# GCC arguments:
# --nostdlib: Do not use system startup files or libraries when linking. Thus, 
#  only the libaries that are explictly specified on the command line are 
#  linked.
SGX_LFLAGS_T = $(SGX_COMMON_CFLAGS) -nostdlib -L$(SGX_LIBRARY_PATH) $(_Other_Link_Flags) \
	-Wl,--whole-archive -l$(Trts_Library_Name) -Wl,--no-whole-archive
ifeq ($(TCMALLOC), Y)
SGX_LFLAGS_T += -Wl,--whole-archive -lsgx_tcmalloc -Wl,--no-whole-archive
endif
SGX_LFLAGS_T += -Wl,--start-group -lsgx_tcxx -lsgx_tstdc -l$(Crypto_Library_Name) -l$(Service_Library_Name) $(_Other_Enclave_Libs) -Wl,--end-group \
	-Wl,-Bstatic -Wl,-Bsymbolic -Wl,--no-undefined \
	-Wl,-pie,-eenclave_entry -Wl,--export-dynamic  \
	-Wl,--defsym,__ImageBase=0 \
	-Wl,--gc-sections \
	-Wl,--version-script=Enclave.lds

define format-rust
	output=$$(cargo fmt -- --check 2>&1); retval=$$?; \
		if [[ $$retval -eq 1 ]]; then \
			$(ECHO) "$$output"; \
			cargo fmt; \
			$(ECHO) "$(GREEN)\nRust code format corrected.$(NO_COLOR)"; \
		fi
endef

define format-check-rust
	output=$$(cargo fmt -- --check 2>&1); retval=$$?; \
		if [[ $$retval -eq 1 ]]; then \
			$(ECHO) "$(RED)\nSome format issues of Rust code are detected:$(NO_COLOR)"; \
			$(ECHO) "\n$$output"; \
			$(ECHO) "\nTo get rid of the format warnings above, run $(CYAN)"make format"$(NO_COLOR) to correct"; \
		fi
endef
