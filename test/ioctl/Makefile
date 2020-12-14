include ../test_common.mk

SGX_SDK ?= /opt/intel/sgxsdk

EXTRA_C_FLAGS := -I$(SGX_SDK)/include

ifdef OCCLUM_DISABLE_DCAP
EXTRA_C_FLAGS += -DOCCLUM_DISABLE_DCAP
endif

EXTRA_LINK_FLAGS :=
BIN_ARGS :=
