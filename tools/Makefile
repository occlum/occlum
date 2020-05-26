SGX_MODE ?= HW

ifneq ($(SGX_MODE), HW)
	BUILD_DIR := build_sim
else
	BUILD_DIR := build
endif

.PHONY: all format format-check clean

all:
	@mkdir -p ../$(BUILD_DIR)/bin/
	@ln -s -f ../../tools/occlum ../$(BUILD_DIR)/bin/occlum
	@ln -s -f ../../tools/occlum-build-enclave ../$(BUILD_DIR)/bin/occlum-build-enclave
	@ln -s -f ../../tools/occlum-gen-default-occlum-json ../$(BUILD_DIR)/bin/occlum-gen-default-occlum-json
	@$(MAKE) --no-print-directory -C protect-integrity

format:
	@$(MAKE) --no-print-directory -C protect-integrity format

format-check:
	@$(MAKE) --no-print-directory -C protect-integrity format-check

clean:
	@$(MAKE) --no-print-directory -C protect-integrity clean
