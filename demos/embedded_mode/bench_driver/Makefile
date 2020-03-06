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
	OCCLUM_BUILD := build_sim
else
	URTS_LIBRARY_NAME := sgx_urts
	UAE_SERVICE_LIBRARY_NAME := sgx_uae_service
	OCCLUM_BUILD := build
endif

C_FLAGS := -Wall \
	-I$(SGX_SDK)/include \
	-I$(OCCLUM_PREFIX)/include
LINK_FLAGS := $(C_FLAGS) -lpthread \
	-L$(SGX_SDK)/lib64 -l$(URTS_LIBRARY_NAME) -l$(UAE_SERVICE_LIBRARY_NAME) -lsgx_uprotected_fs \
	-L$(OCCLUM_PREFIX)/$(OCCLUM_BUILD)/lib -locclum-pal

ALL_BUILD_SUBDIRS := $(sort $(patsubst %/,%,$(dir $(BIN) $(C_OBJS))))

.PHONY: all clean

all: $(BIN)

$(BIN) $(C_OBJS): $(ALL_BUILD_SUBDIRS)

$(BIN): $(C_OBJS)
	@$(CC) $(C_OBJS) -o $@ $(LINK_FLAGS)
	@echo "LINK => $@"

$(BUILD_DIR)/%.o: %.c
	@$(CC) $(C_FLAGS) -c $< -o $@
	@echo "CC <= $@"

$(ALL_BUILD_SUBDIRS):
	@mkdir -p $@

clean:
	@-$(RM) -rf $(BUILD_DIR)
