# Run FISH script on Occlum

This demo will show Occlum's support in shell script.

Occlum now only supports FISH (the friendly interactive shell, https://github.com/fish-shell/fish-shell) for now
because FISH initially use `posix_spawn()` to create process.

This shell script works with BusyBox (the Swiss army knife of embedded Linux, https://busybox.net/).
BusyBox combines tiny versions of many common UNIX utilities into a single small executable. It provides replacements
for most of the utilities you usually find in GNU fileutils, shellutils, etc.

This shell script contains executable binaries, pipe symbols and output redirection like this:
```
busybox echo "Hello-world-from-fish" | busybox awk '$1=$1' FS="-" OFS=" " > /root/output.txt
```

which is defined in `fish_script.sh`. `awk` will replace `-` to `space` and should output result
string `Hello world from fish` and store in `/root/output.txt` of Occlum SEFS and can only be read
inside Occlum.

## Step 1:
Downlaod FISH and busybox and build them with Occlum tool chain:
```
./download_and_build.sh
```

## Step 2:
Prepare environment by running:
```
./env_setup.sh
```

If user wants to run this demo on non-SGX platform, run command:
```
SGX_MODE=SIM ./env_setup.sh
```

## Step 3:
Run command to execute script:
```
cd occlum-context && occlum run /bin/fish /fish_script.sh
```

## Step 4:
Go to `occlum-context` and check result by running:
```
occlum run /bin/busybox cat /root/output.txt
```

And you should see `Hello world from fish`.
