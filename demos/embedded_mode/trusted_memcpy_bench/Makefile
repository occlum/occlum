# Program targeted for Occlum need to be built with the Occlum toolchain
CC := occlum-gcc

BUILD_DIR := build
BIN := $(BUILD_DIR)/trusted_memcpy_bench

C_SRCS := $(wildcard *.c)
C_OBJS := $(addprefix $(BUILD_DIR)/,$(C_SRCS:.c=.o))
C_FLAGS := -Wall
LINK_FLAGS := $(C_FLAGS)

ALL_BUILD_SUBDIRS := $(sort $(patsubst %/,%,$(dir $(BIN) $(C_OBJS))))

.PHONY: all test clean

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
