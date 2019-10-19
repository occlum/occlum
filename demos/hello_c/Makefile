CC := occlum-gcc
CFLAGS := -fPIC -pie

.PHONY: all clean

all: hello_world

hello_world: hello_world.c
	$(CC) $(CFLAGS) $^ -o $@

clean:
	rm -rf hello_world
