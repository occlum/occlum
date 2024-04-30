include ../sgxenv.mk

LIBOCCLUM_PAL_SO := $(BUILD_DIR)/lib/libocclum-pal.so
LIBOCCLUM_PAL_SONAME := libocclum-pal.so.$(MAJOR_VER_NUM)

ifeq ($(SGX_MODE), HW)
	LIBOCCLUM_PAL_SO_REAL := $(BUILD_DIR)/lib/libocclum-pal.so.$(VERSION_NUM)
else ifeq ($(SGX_MODE), HYPER)
	LIBOCCLUM_PAL_SO_REAL := $(BUILD_DIR)/lib/libocclum-pal_hyper.so.$(VERSION_NUM)
	OCCLUM_DISABLE_DCAP := 1
else
	LIBOCCLUM_PAL_SO_REAL := $(BUILD_DIR)/lib/libocclum-pal_sim.so.$(VERSION_NUM)
endif

# A dependency on Rust SGX SDK
LIBSGX_USTDC_A := $(BUILD_DIR)/lib/libsgx_ustdc.a

EDL_C_SRCS := $(addprefix $(OBJ_DIR)/pal/$(SRC_OBJ)/,Enclave_u.c Enclave_u.h)
EDL_C_OBJS := $(addprefix $(OBJ_DIR)/pal/$(SRC_OBJ)/,Enclave_u.o)
C_SRCS := $(sort $(wildcard src/*.c src/*/*.c))
CXX_SRCS := $(sort $(wildcard src/*.cpp src/*/*.cpp))
C_OBJS := $(addprefix $(OBJ_DIR)/pal/,$(C_SRCS:.c=.o))
CXX_OBJS := $(addprefix $(OBJ_DIR)/pal/,$(CXX_SRCS:.cpp=.o))
VDSO_SRCS := $(CRATES_DIR)/vdso-time/ocalls/vdso-time-ocalls.c
VDSO_OBJS := $(addprefix $(OBJ_DIR)/pal/external_ocalls,$(VDSO_SRCS:.c=.o))

# Object files for simulation mode are stored in pal/src_sim
ifeq ($(SGX_MODE), SIM)
	C_OBJS := $(subst pal/src,pal/src_sim,$(C_OBJS))
	CXX_OBJS := $(subst pal/src,pal/src_sim,$(CXX_OBJS))
else ifeq ($(SGX_MODE), HYPER)
	C_OBJS := $(subst pal/src,pal/src_hyper,$(C_OBJS))
	CXX_OBJS := $(subst pal/src,pal/src_hyper,$(CXX_OBJS))
endif

HEADER_FILES := $(sort $(wildcard src/*.h include/*.h include/*/*.h))

C_COMMON_FLAGS := -I$(OBJ_DIR)/pal/$(SRC_OBJ) -Iinclude -Iinclude/edl
ifdef OCCLUM_DISABLE_DCAP
C_COMMON_FLAGS += -DOCCLUM_DISABLE_DCAP
endif
C_FLAGS := $(C_COMMON_FLAGS) $(SGX_CFLAGS_U)
CXX_FLAGS := $(C_COMMON_FLAGS) $(SGX_CXXFLAGS_U)
LINK_FLAGS := $(SGX_LFLAGS_U) -shared -L$(RUST_SGX_SDK_DIR)/sgx_ustdc/ -lsgx_ustdc -lsgx_uprotected_fs -ldl
LINK_FLAGS += -Wl,--version-script=pal.lds
ifndef OCCLUM_DISABLE_DCAP
LINK_FLAGS += -lsgx_dcap_ql -lsgx_dcap_quoteverify
ifeq ($(SGX_MODE), HW)
LINK_FLAGS += -lsgx_quote_ex
else ifeq ($(SGX_MODE), HYPER)
LINK_FLAGS += -lsgx_quote_ex_hyper
else
LINK_FLAGS += -lsgx_quote_ex_sim
endif
endif

LINK_FLAGS += -L$(PROJECT_DIR)/deps/io-uring/ocalls/target/release/ -lsgx_io_uring_ocalls
ALL_BUILD_SUBDIRS := $(sort $(patsubst %/,%,$(dir $(LIBOCCLUM_PAL_SO_REAL) $(EDL_C_OBJS) $(C_OBJS) $(CXX_OBJS) $(VDSO_OBJS))))

.PHONY: all format format-check clean

all: $(ALL_BUILD_SUBDIRS) $(LIBOCCLUM_PAL_SO_REAL)

$(ALL_BUILD_SUBDIRS):
	@mkdir -p $@

$(LIBOCCLUM_PAL_SO_REAL): $(LIBSGX_USTDC_A) $(EDL_C_OBJS) $(C_OBJS) $(CXX_OBJS) $(VDSO_OBJS)
	@$(CXX) $^ -o $@ $(LINK_FLAGS) -Wl,-soname=$(LIBOCCLUM_PAL_SONAME)
	@# Create symbolic files because occlum run and exec will need it when linking.
	@cd $(BUILD_DIR)/lib && ln -sf $(notdir $(LIBOCCLUM_PAL_SO_REAL)) $(notdir $(LIBOCCLUM_PAL_SONAME)) && \
		ln -sf $(notdir $(LIBOCCLUM_PAL_SONAME)) $(notdir $(LIBOCCLUM_PAL_SO))
	@echo "LINK => $@"

$(OBJ_DIR)/pal/$(SRC_OBJ)/Enclave_u.o: $(OBJ_DIR)/pal/$(SRC_OBJ)/Enclave_u.c
	@$(CC) $(C_FLAGS) -c $< -o $@
	@echo "CC <= $@"

$(OBJ_DIR)/pal/$(SRC_OBJ)/Enclave_u.c: $(SGX_EDGER8R) ../Enclave.edl
	@cd $(OBJ_DIR)/pal/$(SRC_OBJ) && \
		$(SGX_EDGER8R) $(SGX_EDGER8R_MODE) --untrusted $(CUR_DIR)/../Enclave.edl \
		--search-path $(SGX_SDK)/include \
		--search-path $(RUST_SGX_SDK_DIR)/edl/ \
		--search-path $(CRATES_DIR)/vdso-time/ocalls \
		--search-path $(PROJECT_DIR)/deps/io-uring/ocalls
	@echo "GEN <= $@"

$(OBJ_DIR)/pal/$(SRC_OBJ)/%.o: src/%.c
	@$(CC) $(C_FLAGS) -c $< -o $@
	@echo "CC <= $@"

$(OBJ_DIR)/pal/$(SRC_OBJ)/%.o: src/%.cpp
	@$(CXX) $(CXX_FLAGS) -c $< -o $@
	@echo "CXX <= $@"

$(VDSO_OBJS): $(VDSO_SRCS)
	@$(CC) $(C_FLAGS) -c $< -o $@
	@echo "CC <= $@"

$(LIBSGX_USTDC_A):
	@$(MAKE) --no-print-directory -C $(RUST_SGX_SDK_DIR)/sgx_ustdc/ > /dev/null
	@cp $(RUST_SGX_SDK_DIR)/sgx_ustdc/libsgx_ustdc.a $(LIBSGX_USTDC_A)
	@echo "GEN <= $@"

format: $(C_SRCS) $(CXX_SRCS) $(HEADER_FILES)
	@$(C_FORMATTER) $^

format-check: $(C_SRCS) $(CXX_SRCS) $(HEADER_FILES)
	@$(C_FORMATTER) --check $^

clean:
	@-$(RM) -f $(BUILD_DIR)/lib/$(LIBOCCLUM_PAL_SONAME) $(LIBOCCLUM_PAL_SO) $(LIBOCCLUM_PAL_SO_REAL) $(LIBSGX_USTDC_A)
	@-$(RM) -rf $(OBJ_DIR)/pal
