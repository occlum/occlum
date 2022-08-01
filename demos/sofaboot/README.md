# A Simple SOFABoot usage demo

This project demonstrates how to execute an unmodified sofaboot projects with Occlum.

1. Download and build sofaboot project

* Use openjdk 8
```
./download_compile_sofaboot.sh jdk8
```

* Use openjdk 11
```
./download_compile_sofaboot.sh
```


2. Run `sofaboot sample standard web` on Occlum

* Run with openjdk 8
```
./run_sofaboot_on_occlum_jdk8.sh
```

* Run with openjdk 11
```
./run_sofaboot_on_occlum.sh
```

3. Test the web services
```
curl http://localhost:8080/actuator/versions
```
