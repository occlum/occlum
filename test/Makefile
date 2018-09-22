CUR_DIR := $(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))
PROJECT_DIR := $(realpath $(CUR_DIR)/../)

TEST_SUITES := hello_world_raw spawn_and_wait4_raw
BUILD_TEST_SUITES := $(TEST_SUITES:%=%)
RUN_TEST_SUITES := $(TEST_SUITES:%=run-%)
CLEAN_TEST_SUITES := $(TEST_SUITES:%=clean-%)

.PHONY: all build test clean $(BUILD_TEST_SUITES) $(RUN_TEST_SUITES) $(CLEAN_TEST_SUITES)

#############################################################################
# Build tests
#############################################################################

all: build

build: build_rusgx_stub $(BUILD_TEST_SUITES)

build_rusgx_stub:
	@$(MAKE) -C rusgx_stub

$(BUILD_TEST_SUITES): %:
	@$(MAKE) -C $@

#############################################################################
# Run tests
#############################################################################

run: build $(RUN_TEST_SUITES)

pal: $(PROJECT_DIR)/src/pal/pal
	cp $< pal

librusgx.signed.so: $(PROJECT_DIR)/src/libos/librusgx.signed.so
	cp $< librusgx.signed.so

$(RUN_TEST_SUITES): run-%: % pal librusgx.signed.so
	@$(MAKE) -C $< run

#############################################################################
# Misc
#############################################################################

clean: $(CLEAN_TEST_SUITES)
	@$(MAKE) -C rusgx_stub clean
	@$(RM) -f pal librusgx.signed.so

$(CLEAN_TEST_SUITES): clean-%:
	@$(MAKE) -C $(patsubst clean-%,%,$@) clean
