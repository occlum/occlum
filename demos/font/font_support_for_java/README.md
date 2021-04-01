# Support font with Java

This project demonstrates how Occlum support font with a Java demo in SGX enclaves.

# About JDK

Both the unmodified [OpenJDK 11](https://hg.openjdk.java.net/portola/jdk11), which is imported from Alpine Linux, and the [Alibaba Dragonwell](https://github.com/alibaba/dragonwell11/tree/dragonwell-for-enclave), which is a downstream version of OpenJDK, are supported now. We have already installed OpenJDK and Dragonwell while building the Docker image, the OpenJDK is installed at `/opt/occlum/toolchains/jvm/java-11-openjdk`, and the Dragonwell is installed at `/opt/occlum/toolchains/jvm/java-11-alibaba-dragonwell`.

Our demos use Dragonwell as the default JDK, you are free to change to OpenJDK by setting the `JAVA_HOME` to point to the installation directory of OpenJDK and copying it into Occlum instance.

## Demo: Java excel file writting with Poi

We provide an excel file writting demo to show how to make occlum support font inside SGX enclaves.

### Create demo and build it

The excel writting demo is created in `create_java_font_app.sh`, and the script also creates a `build.gradle` to build demo. Then a docker container is created to build the demo and we will get a fat jar file. The docker container is based on Alpine, we will collect all dependent libs and font in document font-lib from the container.

### Run demo in Occlum container

Create occlum instance and copy all libs into occlum image which font depends in occlum container, then build the occlum image and run the demo.

### How to Run

Step 1: Create and build demo, it also collects the font dependencies
```
./create_java_font_app.sh
```

Step 2: Create occlum instance and run the demo in occlum's latest docker image
```
docker run -it --device /dev/isgx --rm -v `pwd`:`pwd` -w `pwd` occlum/occlum:[latest version]-ubuntu18.04 `pwd`/run_java_font_app_internal.sh
```

Step 3: To check whether it works, a `Demo.xlsx` file should be created in the host path of occlum instance.
