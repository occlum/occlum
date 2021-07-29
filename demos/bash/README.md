# Run Bash script on Occlum

In this demo, we will show how to run a Bash script inside Occlum.

Bash is the most widely used shell implementation around the world. Previously, we didn't support Bash because of too many technical challenges, such as compilation, lack of fork and execve system calls, etc.

Now, Bash is finally supported with modification to the source code of Bash. We have evaluated and all commands defined in `occlum_bash_test.sh` are all supported.

Please follow below steps to run this demo:

1. Download and build Busybox and Occlum-version Bash
```
./prepare_bash_demo.sh
```

2. Run Bash script in Occlum
```
./run_bash_demo.sh
```
