# A Simple SOFABoot usage demo

This project demonstrates how to execute an unmodified sofaboot projects with Occlum.

1. Download and build sofaboot project
```
./download_compile_sofaboot.sh
```

2. Run `sofaboot sample standard web` on Occlum
```
./run_sofaboot_on_occlum.sh
```

3. Test the web services
```
curl http://localhost:8080/actuator/versions
```
