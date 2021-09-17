SGX_MODE ?= HW
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

format:
	@$(MAKE) --no-print-directory -C protect-integrity format

format-check:
	@$(MAKE) --no-print-directory -C protect-integrity format-check

clean:
	@$(MAKE) --no-print-directory -C protect-integrity clean
	@$(MAKE) --no-print-directory -C gen_internal_conf clean
	@$(MAKE) --no-print-directory -C copy_bom clean
	@$(MAKE) --no-print-directory -C init clean
