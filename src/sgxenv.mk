MAIN_MAKEFILE := $(firstword $(MAKEFILE_LIST))
INCLUDE_MAKEFILE := $(lastword $(MAKEFILE_LIST))
CUR_DIR := $(shell dirname $(realpath $(MAIN_MAKEFILE)))
PROJECT_DIR := $(realpath $(CUR_DIR)/../../)
BUILD_DIR := $(PROJECT_DIR)/build

SGX_SDK ?= /opt/intel/sgxsdk
SGX_MODE ?= HW
SGX_ARCH ?= x64

ifeq ($(shell getconf LONG_BIT), 32)
	SGX_ARCH := x86
else ifeq ($(findstring -m32, $(CXXFLAGS)), -m32)
	SGX_ARCH := x86
endif

SGX_COMMON_CFLAGS := -Wall

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

ifeq ($(SGX_DEBUG), 1)
ifeq ($(SGX_PRERELEASE), 1)
$(error Cannot set SGX_DEBUG and SGX_PRERELEASE at the same time!!)
endif
endif

ifeq ($(SGX_DEBUG), 1)
	SGX_COMMON_CFLAGS += -O0 -g
else
	SGX_COMMON_CFLAGS += -O2
endif

RUST_SGX_SDK_DIR := $(PROJECT_DIR)/deps/rust-sgx-sdk
SGX_COMMON_CFLAGS += -I$(RUST_SGX_SDK_DIR)/common/ -I$(RUST_SGX_SDK_DIR)/edl/

ifneq ($(SGX_MODE), HW)
	Urts_Library_Name := sgx_urts_sim
else
	Urts_Library_Name := sgx_urts
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
SGX_CFLAGS_U := $(SGX_COMMON_CFLAGS) -fPIC -Wno-attributes -I$(SGX_SDK)/include
SGX_CXXFLAGS_U := $(SGX_CFLAGS_U) -std=c++11

SGX_LFLAGS_U := $(SGX_COMMON_CFLAGS) -lpthread -L$(SGX_LIBRARY_PATH) -l$(Urts_Library_Name)
ifneq ($(SGX_MODE), HW)
	SGX_LFLAGS_U += -lsgx_uae_service_sim
else
	SGX_LFLAGS_U += -lsgx_uae_service
endif

#
# Export flags used to compile or link untrusted modules
#
SGX_CFLAGS_T := $(SGX_COMMON_CFLAGS) -nostdinc -fvisibility=hidden -fpie -fstack-protector \
	-I$(RUST_SGX_SDK_DIR)/common/inc/ -I$(SGX_SDK)/include -I$(SGX_SDK)/include/tlibc
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
	-Wl,--whole-archive -l$(Trts_Library_Name) -Wl,--no-whole-archive \
	-Wl,--start-group -lsgx_tstdc -lsgx_tcxx -l$(Crypto_Library_Name) -l$(Service_Library_Name) $(_Other_Enclave_Libs) -Wl,--end-group \
	-Wl,-Bstatic -Wl,-Bsymbolic -Wl,--no-undefined \
	-Wl,-pie,-eenclave_entry -Wl,--export-dynamic  \
	-Wl,--defsym,__ImageBase=0 \
	-Wl,--gc-sections \
	-Wl,--version-script=Enclave.lds
