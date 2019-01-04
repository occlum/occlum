CUR_DIR := $(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))
PROJECT_DIR := $(realpath $(CUR_DIR)/../)

TEST_SUITES := empty hello_world malloc getpid spawn
BUILD_TEST_SUITES := $(TEST_SUITES:%=%)
RUN_TEST_SUITES := $(TEST_SUITES:%=run-%)
CLEAN_TEST_SUITES := $(TEST_SUITES:%=clean-%)

.PHONY: all build test clean $(BUILD_TEST_SUITES) $(RUN_TEST_SUITES) $(CLEAN_TEST_SUITES)

#############################################################################
# Build tests
#############################################################################

all: build

build: $(BUILD_TEST_SUITES)

$(BUILD_TEST_SUITES): %:
	@$(MAKE) -C $@

#############################################################################
# Run tests
#############################################################################

run: build $(RUN_TEST_SUITES)

pal: $(PROJECT_DIR)/src/pal/pal
	cp $< pal

libocclum.signed.so: $(PROJECT_DIR)/src/libos/libocclum.signed.so
	cp $< libocclum.signed.so

$(RUN_TEST_SUITES): run-%: % pal libocclum.signed.so
	@$(MAKE) -C $< run

#############################################################################
# Misc
#############################################################################

clean: $(CLEAN_TEST_SUITES)
	@$(RM) -f pal libocclum.signed.so

$(CLEAN_TEST_SUITES): clean-%:
	@$(MAKE) -C $(patsubst clean-%,%,$@) clean
