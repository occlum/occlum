# Run Bash script on Occlum

In this demo, we will show how to run a Bash script inside Occlum.

Bash is the most widely used shell implementation around the world. Previously, we didn't support Bash because of too many technical challenges, such as compilation, lack of fork and execve system calls, etc.

Now, Bash is finally supported with modification to the source code of Bash. We have evaluated and all commands defined in `occlum_bash_test.sh` are all supported.

Two versions [`musl-libc` and `glibc`] of bash demo is provided:

* musl-libc bash demo
```
./run_bash_demo.sh musl
```

* glibc bash demo
```
./run_bash_demo.sh
```
