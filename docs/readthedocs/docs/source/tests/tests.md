# Tests for Occlum

## Unit Tests

There is a set of unit tests in the source [**test**](https://github.com/occlum/occlum/tree/master/test). It includes almost all the syscall (Occlum supported) test cases. Every PR will run this unit tests to make sure of no failures introduced on basic functions.

Users could run the unit test manually as well.

```
// run all the unit test cases for musl-libc
# make test

// run all the unit test cases for glibc
# make test-glibc

// run only specified test case, timerfd for example
# TESTS=timerfd make test

// run test cases for 100 times
# make test times=100

// run test without rebuilding Occlum, using binaries installed already
# OCCLUM_BIN_PATH=/opt/occlum/build/bin make test
```

## Gvisor Tests
<To be added>

## LTP Tests

Linux [LTP](https://github.com/linux-test-project/ltp) is the most popular test suite for Linux. Occlum could also apply the LTP to verify the stability and compatibility to Linux app. For detail, please refer to [**linux-ltp**](https://github.com/occlum/occlum/tree/master/demos/linux-ltp).
