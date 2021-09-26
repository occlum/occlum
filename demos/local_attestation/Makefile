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
	@rm -rf occlum_instance && mkdir -p occlum_instance
	@cd occlum_instance && \
		occlum init && rm -rf image && \
		copy_bom -f ../bom.yaml --root image --include-dir /opt/occlum/etc/template && \
		occlum build
	@cp -t occlum_instance $(APP) $(BUILD_DIR)/libenclave_initiator.signed.so

build_src:
	@$(MAKE) --no-print-directory -C DiffieHellmanLibrary
	@$(MAKE) --no-print-directory -C AppInitiator
	@$(MAKE) --no-print-directory -C AppResponder
	@$(MAKE) --no-print-directory -C EnclaveInitiator

test: build_src
	cd occlum_instance && LD_LIBRARY_PATH=../$(BUILD_DIR):$(SGX_SDK)/sdk_libs RUST_BACKTRACE=1 \
		./appinitiator

clean:
	@$(MAKE) --no-print-directory -C AppInitiator clean
	@$(MAKE) --no-print-directory -C AppResponder  clean
	@$(MAKE) --no-print-directory -C EnclaveInitiator clean
	@$(MAKE) --no-print-directory -C DiffieHellmanLibrary clean
	@rm -rf .occlum occlum_instance $(BUILD_DIR)
