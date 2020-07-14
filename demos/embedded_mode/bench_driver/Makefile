SGX_SDK ?= /opt/intel/sgxsdk
OCCLUM_PREFIX ?= /opt/occlum
SGX_MODE ?= HW

BUILD_DIR := build
BIN := $(BUILD_DIR)/bench_driver

C_SRCS := $(sort $(wildcard *.c))
C_OBJS := $(addprefix $(BUILD_DIR)/,$(C_SRCS:.c=.o))

ifneq ($(SGX_MODE), HW)
	URTS_LIBRARY_NAME := sgx_urts_sim
	UAE_SERVICE_LIBRARY_NAME := sgx_uae_service_sim
	OCCLUM_PAL_LIB := occlum-pal_sim
else
	URTS_LIBRARY_NAME := sgx_urts
	UAE_SERVICE_LIBRARY_NAME := sgx_uae_service
	OCCLUM_PAL_LIB := occlum-pal
endif

MAJOR_VER_NUM = $(shell grep '\#define OCCLUM_MAJOR_VERSION' $(OCCLUM_PREFIX)/include/occlum_version.h |  awk '{print $$3}')
MINOR_VER_NUM = $(shell grep '\#define OCCLUM_MINOR_VERSION' $(OCCLUM_PREFIX)/include/occlum_version.h |  awk '{print $$3}')
PATCH_VER_NUM = $(shell grep '\#define OCCLUM_PATCH_VERSION' $(OCCLUM_PREFIX)/include/occlum_version.h |  awk '{print $$3}')
VERSION_NUM = $(MAJOR_VER_NUM).$(MINOR_VER_NUM).$(PATCH_VER_NUM)

C_FLAGS := -Wall \
	-I$(SGX_SDK)/include \
	-I$(OCCLUM_PREFIX)/include
LINK_FLAGS := $(C_FLAGS) -lpthread \
	-L$(SGX_SDK)/lib64 -l$(URTS_LIBRARY_NAME) -l$(UAE_SERVICE_LIBRARY_NAME) -lsgx_uprotected_fs \
	-L$(BUILD_DIR) -locclum-pal

ALL_BUILD_SUBDIRS := $(sort $(patsubst %/,%,$(dir $(BIN) $(C_OBJS))))

.PHONY: all clean

all: $(BIN)

$(BIN) $(C_OBJS): $(ALL_BUILD_SUBDIRS)

$(BIN): $(C_OBJS) libocclum-pal.so
	@$(CC) $(C_OBJS) -o $@ $(LINK_FLAGS)
	@echo "LINK => $@"

$(BUILD_DIR)/%.o: %.c
	@$(CC) $(C_FLAGS) -c $< -o $@
	@echo "CC <= $@"

libocclum-pal.so: $(ALL_BUILD_SUBDIRS)
	@cd $(BUILD_DIR) && ln -sf $(OCCLUM_PREFIX)/build/lib/lib$(OCCLUM_PAL_LIB).so.$(VERSION_NUM) libocclum-pal.so.$(MAJOR_VER_NUM) && \
		ln -sf libocclum-pal.so.$(MAJOR_VER_NUM) libocclum-pal.so

$(ALL_BUILD_SUBDIRS):
	@mkdir -p $@

clean:
	@-$(RM) -rf $(BUILD_DIR)
