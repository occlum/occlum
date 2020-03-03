SGX_MODE ?= HW

ifneq ($(SGX_MODE), HW)
	BUILD_DIR := build_sim
else
	BUILD_DIR := build
endif

.PHONY: all clean

all:
	@mkdir -p ../$(BUILD_DIR)/bin/
	@ln -s -f ../../tools/occlum ../$(BUILD_DIR)/bin/occlum
	@ln -s -f ../../tools/occlum-build-enclave ../$(BUILD_DIR)/bin/occlum-build-enclave
	@ln -s -f ../../tools/occlum-gen-default-occlum-json ../$(BUILD_DIR)/bin/occlum-gen-default-occlum-json
	@$(MAKE) --no-print-directory -C protect-integrity

clean:
	@$(MAKE) --no-print-directory -C protect-integrity clean
