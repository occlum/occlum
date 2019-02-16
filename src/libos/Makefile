LIB_NAME := librusgx.a
SRCS := $(wildcard src/*.rs)

.PHONY: all clean

all: $(LIB_NAME)

$(LIB_NAME): $(SRCS)
	RUSTC_BOOTSTRAP=1 cargo build --release
	cp ./target/release/$(LIB_NAME) $(LIB_NAME)

clean:
	@cargo clean
	@-$(RM) $(LIB_NAME)
