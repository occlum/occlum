CC := occlum-gcc

.PHONY: all clean

all: hello_world

hello_world: hello_world.c
	$(CC) $^ -o $@

clean:
	rm -rf hello_world
