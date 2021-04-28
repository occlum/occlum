include ../test_common.mk

SGX_SDK ?= /opt/intel/sgxsdk

EXTRA_C_FLAGS := -I$(SGX_SDK)/include -g

ifdef OCCLUM_DISABLE_DCAP
EXTRA_C_FLAGS += -DOCCLUM_DISABLE_DCAP
endif

## TODO: support DCAP test in SIM mode
ifeq ($(SGX_MODE), SIM)
EXTRA_C_FLAGS += -DOCCLUM_DISABLE_DCAP
else ifeq ($(SGX_MODE), SW)
EXTRA_C_FLAGS += -DOCCLUM_DISABLE_DCAP
endif

EXTRA_LINK_FLAGS :=
BIN_ARGS :=
