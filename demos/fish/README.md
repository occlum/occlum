# Run FISH script on Occlum

This demo will show Occlum's support in shell script.

Occlum now only supports FISH (the friendly interactive shell, https://github.com/fish-shell/fish-shell) for now
because FISH initially use `posix_spawn()` to create process.

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

## Step 1:
Downlaod FISH and busybox and build them with Occlum tool chain:
```
./download_and_build.sh
```

## Step 2:
Run command to prepare context and execute script:
```
./run_fish_test.sh
```
Or if this demo is running on non-SGX platform, use:
```
SGX_MODE=SIM ./run_fish_test.sh
```

And you should see `Hello world from fish`.
