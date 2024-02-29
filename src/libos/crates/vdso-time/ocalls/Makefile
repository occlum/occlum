all: libs

libs: libvdso_time_ocalls.a

libvdso_time_ocalls.a: vdso_time_ocalls.o
	ar rcs $@ $^

vdso_time_ocalls.o: vdso-time-ocalls.c
	gcc -O3 -c -o $@ $<	

clean:
	rm -f *.o *.a