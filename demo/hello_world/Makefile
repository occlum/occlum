CC := occlum-gcc
CFLAGS := -fPIC -pie

.PHONY: all test test-native clean

all: hello_world

hello_world: hello_world.c
	$(CC) $(CFLAGS) $^ -o $@

# Run hello_world on Occlum inside an SGX enclave
test: hello_world
	rm -rf occlum_workspace
	mkdir occlum_workspace
	cd occlum_workspace && \
		occlum init && \
		cp ../hello_world image/bin && \
		occlum build && \
		occlum run /bin/hello_world

# Run hello_world on the native OS (e.g., Linux)
test-native:
	./hello_world

clean:
	rm -rf hello_world occlum_workspace
