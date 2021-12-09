.PHONY: all clean

src = $(wildcard *.c)
hdr = $(wildcard *.h)
target = $(patsubst %.c, %, ${src})

all: ${target}
%:%.c ${hdr}
	gcc $^ -o $@ -g

clean:
	rm -rf ${target}

