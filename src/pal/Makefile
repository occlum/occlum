include ../sgxenv.mk

LIBOCCLUM_PAL_SO := $(BUILD_DIR)/lib/libocclum-pal.so

# A dependency on Rust SGX SDK
LIBSGX_USTDC_A := $(BUILD_DIR)/lib/libsgx_ustdc.a

EDL_C_SRCS := $(addprefix $(BUILD_DIR)/src/pal/src/,Enclave_u.c Enclave_u.h)
EDL_C_OBJS := $(addprefix $(BUILD_DIR)/src/pal/src/,Enclave_u.o)
C_SRCS := $(sort $(wildcard src/*.c src/*/*.c))
C_OBJS := $(addprefix $(BUILD_DIR)/src/pal/,$(C_SRCS:.c=.o))
CXX_SRCS := $(sort $(wildcard src/*.cpp src/*/*.cpp))
CXX_OBJS := $(addprefix $(BUILD_DIR)/src/pal/,$(CXX_SRCS:.cpp=.o))
HEADER_FILES := $(sort $(wildcard include/*.h include/*/*.h))

C_COMMON_FLAGS := -I$(BUILD_DIR)/src/pal/src -Iinclude -Iinclude/edl
C_FLAGS := $(C_COMMON_FLAGS) $(SGX_CFLAGS_U)
CXX_FLAGS := $(C_COMMON_FLAGS) $(SGX_CXXFLAGS_U)
LINK_FLAGS := $(SGX_LFLAGS_U) -shared -L$(RUST_SGX_SDK_DIR)/sgx_ustdc/ -lsgx_ustdc -lsgx_uprotected_fs
LINK_FLAGS += -Wl,--version-script=pal.lds

ALL_BUILD_SUBDIRS := $(sort $(patsubst %/,%,$(dir $(LIBOCCLUM_PAL_SO) $(EDL_C_OBJS) $(C_OBJS) $(CXX_OBJS))))

.PHONY: all format format-check clean

all: $(ALL_BUILD_SUBDIRS) $(LIBOCCLUM_PAL_SO)

$(ALL_BUILD_SUBDIRS):
	@mkdir -p $@

$(LIBOCCLUM_PAL_SO): $(LIBSGX_USTDC_A) $(EDL_C_OBJS) $(C_OBJS) $(CXX_OBJS)
	@$(CXX) $^ -o $@ $(LINK_FLAGS)
	@echo "LINK => $@"

$(BUILD_DIR)/src/pal/src/Enclave_u.o: $(BUILD_DIR)/src/pal/src/Enclave_u.c
	@$(CC) $(C_FLAGS) -c $< -o $@
	@echo "CC <= $@"

$(BUILD_DIR)/src/pal/src/Enclave_u.c: $(SGX_EDGER8R) ../Enclave.edl
	@cd $(BUILD_DIR)/src/pal/src && \
		$(SGX_EDGER8R) --untrusted $(CUR_DIR)/../Enclave.edl \
		--search-path $(SGX_SDK)/include \
		--search-path $(RUST_SGX_SDK_DIR)/edl/
	@echo "GEN <= $@"

$(BUILD_DIR)/src/pal/%.o: %.c
	@$(CC) $(C_FLAGS) -c $< -o $@
	@echo "CC <= $@"

$(BUILD_DIR)/src/pal/%.o: %.cpp
	@$(CXX) $(CXX_FLAGS) -c $< -o $@
	@echo "CXX <= $@"

$(LIBSGX_USTDC_A):
	@$(MAKE) --no-print-directory -C $(RUST_SGX_SDK_DIR)/sgx_ustdc/ > /dev/null
	@cp $(RUST_SGX_SDK_DIR)/sgx_ustdc/libsgx_ustdc.a $(LIBSGX_USTDC_A)
	@echo "GEN <= $@"

format: $(C_SRCS) $(CXX_SRCS) $(HEADER_FILES)
	@$(C_FORMATTER) $^

format-check: $(C_SRCS) $(CXX_SRCS) $(HEADER_FILES)
	@$(C_FORMATTER) --check $^

clean:
	@-$(RM) -f $(LIBOCCLUM_PAL_SO) $(LIBSGX_USTDC_A) $(C_OBJS) $(CXX_OBJS) $(EDL_C_OBJS) $(EDL_C_SRCS)
