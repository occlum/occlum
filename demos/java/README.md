# Use Java with Occlum

This project demonstrates how Occlum enables _unmodified_ Java programs running in SGX enclaves.

# About JDK

Both the unmodified [OpenJDK 11](https://hg.openjdk.java.net/portola/jdk11), which is imported from Alpine Linux, and the [Alibaba Dragonwell](https://github.com/alibaba/dragonwell11/tree/dragonwell-for-enclave), which is a downstream version of OpenJDK, are supported now. We have already installed OpenJDK and Dragonwell while building the Docker image, the OpenJDK is installed at `/opt/occlum/toolchains/jvm/java-11-openjdk`, and the Dragonwell is installed at `/opt/occlum/toolchains/jvm/java-11-alibaba-dragonwell`.

Our demos use Dragonwell as the default JDK, you are free to change to OpenJDK by setting the `JAVA_HOME` to point to the installation directory of OpenJDK and copying it into Occlum instance.

## Demo: Hello World

We provide a "Hello World" demo to show how to run a simple Java program inside SGX enclaves. The demo code can be found [here](hello_world/).

### How to Run

Step 1: Compile the source code with `occlum-javac`
```
occlum-javac ./hello_world/Main.java
```
When completed, the resulting file can be found at `./hello_world/Main.class`.

Step 2: Start JVM to run the hello world program
```
./run_java_on_occlum.sh hello
```

## Demo: Web application with Spring Boot

We also choose a Java web application that using WebSocket with [Spring Boot](https://spring.io/projects/spring-boot). The demo code can be found [here](https://github.com/spring-guides/gs-messaging-stomp-websocket).

### How to Run

Step 1: Download the demo code and build a Fat JAR file with Maven
```
./download_and_build_web_app.sh
```
When completed, the resulting Fat JAR file can be found at `./gs-messaging-stomp-websocket/complete/target/gs-messaging-stomp-websocket-0.1.0.jar`.

Step 2: Start JVM to run the JAR file on Occlum
```
./run_java_on_occlum.sh web_app
```
The web application should now start to listen on port 8080 and serve requests.

Step 3: To check whether it works, run
```
curl http://localhost:8080
```
in another terminal.

It is recommended to access the web application in a Web browser. You have to manually map port 8080 of the Docker container to a port on the host OS. Check out how to use [the `-p` argument of `docker run` command](https://docs.docker.com/engine/reference/commandline/run/).

# Demo: ProcessBuilder application
This demo shows that Occlum has enabled support for `ProcessBuilder` class and multiprocess in Java.

# How to Run
Step 1: Compile the source code with `occlum-javac`
```
occlum-javac ./processBuilder/processBuilder.java
```
When completed, the resulting file can be found at `./processBuilder/processBuilder.java`.

Try to run it on native Linux with:
```
cd processBuilder && occlum-java processBuilder
```

Step 2: Start JVM to run the processBuilder demo
```
./run_java_on_occlum.sh processBuilder
```
