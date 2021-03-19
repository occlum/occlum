#!/bin/bash
bench_dir=$PWD

run_async_file_in_host(){
   cd $bench_dir/../../target/release/examples/
   ./read_write_bench $file_num $block_size $file_total_size $is_read $is_seq $use_fsync $use_direct $loops
   sleep 2
}

run_async_file_in_sgx(){
   cd $bench_dir/../examples/sgx/read_write_bench/bin
   ./app
   sleep 2
}

run(){
   if [ $sgx ];
   then
      run_async_file_in_sgx
   else
      run_async_file_in_host
   fi
}

cd $bench_dir/..
cargo b --release --examples

cd $bench_dir/../examples/sgx/read_write_bench
make

file_num=1
file_total_size=100
loops=100
use_fsync="false"
use_direct="false"

is_read="true"
{
    is_seq="true"
    use_direct="false"
    for block_size in 4 8 12 16 20 24 28 32
    do
      run
    done

    is_seq="false"
    use_direct="false"
    for block_size in 4 8 12 16 20 24 28 32
    do
       run
    done

    is_seq="true"
    use_direct="true"
    for block_size in 4 8 12 16 20 24 28 32
    do
       run
    done

    is_seq="false"
    use_direct="true"
    for block_size in 4 8 12 16 20 24 28 32
    do
       run
    done
}

is_read="false"
{
    is_seq="true"
    use_direct="false"
    use_fsync="false"
    for block_size in 4 8 12 16 20 24 28 32
    do
       run
    done

    is_seq="false"
    use_direct="false"
    use_fsync="false"
    for block_size in 4 8 12 16 20 24 28 32
    do
       run
    done

    is_seq="true"
    use_direct="false"
    use_fsync="true"
    for block_size in 4 8 12 16 20 24 28 32
    do
       run
    done

    is_seq="false"
    use_direct="false"
    use_fsync="true"
    for block_size in 4 8 12 16 20 24 28 32
    do
       run
    done

    is_seq="true"
    use_direct="true"
    use_fsync="false"
    for block_size in 4 8 12 16 20 24 28 32
    do
       run
    done

    is_seq="false"
    use_direct="true"
    use_fsync="false"
    for block_size in 4 8 12 16 20 24 28 32
    do
       run
    done
}
