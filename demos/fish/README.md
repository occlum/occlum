# Run FISH script on Occlum

This demo will show Occlum's support in shell script.

Occlum now only supports FISH (the friendly interactive shell, https://github.com/fish-shell/fish-shell) for now
because FISH initially use `posix_spawn()` to create process.

## 1. Run a simple FISH script with BusyBox

This shell script works with BusyBox (the Swiss army knife of embedded Linux, https://busybox.net/).
BusyBox combines tiny versions of many common UNIX utilities into a single small executable. It provides replacements
for most of the utilities you usually find in GNU fileutils, shellutils, etc.

This shell script contains executable binaries, pipe symbols and output redirection like this:
```
command echo "Hello-world-from-fish" | awk '$1=$1' FS="-" OFS=" " > /root/output.txt
cat /root/output.txt
```

which is defined in `fish_script.sh`. `awk` will replace `-` to `space` and should output result
string `Hello world from fish` and store in `/root/output.txt` of Occlum SEFS and can only be read
inside Occlum. `echo`, `awk`, `cat` here are actually symbolic files linked to busybox and in this way, we don't need
to write `busybox` prefix. The `command` keyword tells FISH that `echo` is an external command because FISH also provides
builtin `echo` command.

The script can be executed by Occlum directly as shown below:
```
occlum run /bin/fish_script.sh
```
As demonstrated here, Occlum supports executing any script file that begins with a [shebang](https://en.wikipedia.org/wiki/Shebang_(Unix))
at its first line by invoking the interpreter program specified with the shebang.

### Step 1:
Downlaod FISH and busybox and build them with Occlum tool chain:
```
./download_and_build.sh
```

### Step 2:
Run command to prepare context and execute script:
```
./run_fish_test.sh
```
Or if this demo is running on non-SGX platform, use:
```
SGX_MODE=SIM ./run_fish_test.sh
```

And you should see `Hello world from fish`.


## Per-Process Resource Configuration with help of FISH

Resource configuration for application running in Occlum is done only in `Occlum.json`. And only default size (mmap, heap, stack) can be
configured. Since Occlum will claim all the memory space at initializtion, if an application doesn't really need the size as big as defined
in `Occlum.json`, the exceeding memory space is wasted. If two applications are running, one of which needs only a small amount of space while
the other needs a lot more, it is better to run with per-process resource configuration.

We achieve this with help of `prlimit` syscall (https://man7.org/linux/man-pages//man2/prlimit.2.html) and FISH shell built-in command
`ulimit` (https://fishshell.com/docs/current/cmds/ulimit.html). Thus, the application must be run in shell script. An example could be like this:

```shell
#! /usr/bin/fish
ulimit -a

# ulimit defined below will override configuration in Occlum.json
ulimit -Ss 10240 # stack size 10M
ulimit -Sd 40960 # heap size 40M
ulimit -Sv 102400 # virtual memory size 100M (including heap, stack, mmap size)

echo "ulimit result:"
ulimit -a

# Run applications with the new resource limits
...
```

Below steps illustrate this usage:

### step 1:
Run command:
```shell
./run_per_process_config_test.sh --without-ulimit
```

This test will fail because `ulimit` commands are commented out and the default memory size defined in Occlum.json is too small for application to run.

### step 2:
Run command:
```shell
./run_per_process_config_test.sh
```
With the resource limits updated by `ulimit` command, the test can now pass.
