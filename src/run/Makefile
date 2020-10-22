include ../sgxenv.mk

BIN := $(BUILD_DIR)/bin/occlum-run

C_SRCS := $(sort $(wildcard *.c))
C_OBJS := $(addprefix $(OBJ_DIR)/run/,$(C_SRCS:.c=.o))

C_COMMON_FLAGS := -I$(PROJECT_DIR)/src/pal/include
C_FLAGS := $(C_COMMON_FLAGS) $(SGX_CFLAGS_U)
LINK_FLAGS := $(SGX_COMMON_CFLAGS) -L$(SGX_LIBRARY_PATH) -L$(BUILD_DIR)/lib -lsgx_uprotected_fs -locclum-pal

ALL_BUILD_SUBDIRS := $(sort $(patsubst %/,%,$(dir $(BIN) $(C_OBJS))))

.PHONY: all format format-check clean

all: $(ALL_BUILD_SUBDIRS) $(BIN)

$(ALL_BUILD_SUBDIRS):
	@mkdir -p $@

$(BIN): $(C_OBJS)
	@$(CC) $^ -o $@ $(LINK_FLAGS)
	@echo "LINK => $@"

$(OBJ_DIR)/run/%.o: %.c
	@$(CC) $(C_FLAGS) -c $< -o $@
	@echo "CC <= $@"

format: $(C_SRCS) $(CXX_SRCS)
	@$(C_FORMATTER) $^

format-check: $(C_SRCS) $(CXX_SRCS)
	@$(C_FORMATTER) --check $^

clean:
	@-$(RM) -f $(BIN) $(C_OBJS)
