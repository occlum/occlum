# Use Java with Occlum

This project demonstrates how Occlum enables _unmodified_ Java programs running in SGX enclaves.

# About JDK

JDK 11 is supported currently. The source code of JDK 11 can be found [here](https://hg.openjdk.java.net/portola/jdk11). In order for it to cooperate with Occlum, a [minor modification](../../tools/toolchains/java/) has been made to it. The modified JDK is compiled in Alpine Linux with `bash configure && make images` commands. We have installed it at `/opt/occlum/toolchains/jvm/java-11-openjdk/jre` while making the Docker image.

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
