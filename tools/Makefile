SGX_MODE ?= HW
MS_BUFFER ?= 0
BUILD_DIR := build

.PHONY: all format format-check clean

all:
	@mkdir -p ../$(BUILD_DIR)/bin/
	@ln -s -f ../../tools/occlum_build.mk ../$(BUILD_DIR)/bin/occlum_build.mk
	@ln -s -f ../../tools/occlum ../$(BUILD_DIR)/bin/occlum
	@$(MAKE) --no-print-directory -C protect-integrity
	@$(MAKE) --no-print-directory -C gen_internal_conf
	@$(MAKE) --no-print-directory -C copy_bom
	@$(MAKE) --no-print-directory -C init
ifndef OCCLUM_DISABLE_DCAP
	@$(MAKE) --no-print-directory -C init_grpc_ratls
endif
	@$(MAKE) --no-print-directory -C init_aecs

format:
	@$(MAKE) --no-print-directory -C protect-integrity format

format-check:
	@$(MAKE) --no-print-directory -C protect-integrity format-check

clean:
	@$(MAKE) --no-print-directory -C protect-integrity clean
	@$(MAKE) --no-print-directory -C gen_internal_conf clean
	@$(MAKE) --no-print-directory -C copy_bom clean
	@$(MAKE) --no-print-directory -C init clean
ifndef OCCLUM_DISABLE_DCAP
	@$(MAKE) --no-print-directory -C init_grpc_ratls clean
endif
	@$(MAKE) --no-print-directory -C init_aecs clean
