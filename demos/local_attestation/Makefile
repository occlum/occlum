CUR_DIR := $(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))
OPENSSL ?= $(CUR_DIR)/deps/openssl
SGX_SDK ?= /opt/intel/sgxsdk

BUILD_DIR := build
APP := $(BUILD_DIR)/appinitiator
SGX_MODE ?= HW

occlum_=$(CUR_DIR)/../..

.PHONY: all build_src test clean

all: occlum_instance

occlum_instance: build_src
	@mkdir -p occlum_instance
	@cd occlum_instance && \
		occlum init && \
		cp $(CUR_DIR)/AppResponder/responder image/bin/ && \
		cp $(OPENSSL)/libcrypto.so.1.1 image/lib/ && \
		cp $(CUR_DIR)/DiffieHellmanLibrary/libecdh.so image/lib/ && \
		occlum build
	@cp -r occlum_instance/.occlum .occlum

build_src:
	@$(MAKE) --no-print-directory -C DiffieHellmanLibrary
	@$(MAKE) --no-print-directory -C AppInitiator
	@$(MAKE) --no-print-directory -C AppResponder
	@$(MAKE) --no-print-directory -C EnclaveInitiator

test: build_src
	LD_LIBRARY_PATH=$(BUILD_DIR):$(SGX_SDK)/sdk_libs RUST_BACKTRACE=1 \
		./$(APP)

clean:
	@$(MAKE) --no-print-directory -C AppInitiator clean
	@$(MAKE) --no-print-directory -C AppResponder  clean
	@$(MAKE) --no-print-directory -C EnclaveInitiator clean
	@$(MAKE) --no-print-directory -C DiffieHellmanLibrary clean
	@rm -rf .occlum occlum_instance $(BUILD_DIR)
