SGX_SDK ?= /opt/intel/sgxsdk
OCCLUM_PREFIX ?= /opt/occlum

BUILD_DIR := build
BIN := $(BUILD_DIR)/bench_driver

C_SRCS := $(sort $(wildcard *.c))
C_OBJS := $(addprefix $(BUILD_DIR)/,$(C_SRCS:.c=.o))

C_FLAGS := -Wall \
	-I$(SGX_SDK)/include \
	-I$(OCCLUM_PREFIX)/include
LINK_FLAGS := $(C_FLAGS) -lpthread \
	-L$(SGX_SDK)/lib64 -lsgx_urts -lsgx_uprotected_fs \
	-L$(OCCLUM_PREFIX)/build/lib -locclum-pal

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
