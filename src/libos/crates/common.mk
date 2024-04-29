MAIN_MAKEFILE := $(firstword $(MAKEFILE_LIST))
INCLUDE_MAKEFILE := $(lastword $(MAKEFILE_LIST))
CURRENT_DIR := $(shell dirname $(realpath $(MAIN_MAKEFILE)))
ROOT_DIR := $(realpath $(shell dirname $(realpath $(INCLUDE_MAKEFILE)))/../../../)
RUST_SGX_SDK_DIR := $(ROOT_DIR)/deps/rust-sgx-sdk
LIBOS_DIR := $(ROOT_DIR)/src/libos
LIBOS_CRATES_DIR := $(LIBOS_DIR)/crates
