#!/bin/bash
run_file_io(){
    ./file-io-bench $thread_num $file_num $block_size $req_merge_num $file_total_size $is_read $is_seq $use_fsync $use_direct $loops
    sleep 2
}

gcc -O2 -pthread file-io-bench.c -o file-io-bench

thread_num=1
file_num=1
req_merge_num=1
file_total_size=100
loops=100

is_read=1
{
    is_seq=1
    use_direct=0
    use_fsync=0
    for block_size in 4 8 12 16 20 24 28 32
    do
       run_file_io
    done

    is_seq=0
    use_direct=0
    use_fsync=0
    for block_size in 4 8 12 16 20 24 28 32
    do
       run_file_io
    done

    is_seq=1
    use_direct=1
    use_fsync=0
    for block_size in 4 8 12 16 20 24 28 32
    do
       run_file_io
    done

    is_seq=0
    use_direct=1
    use_fsync=0
    for block_size in 4 8 12 16 20 24 28 32
    do
       run_file_io
    done
}

is_read=0
{
    is_seq=1
    use_direct=0
    use_fsync=0
    for block_size in 4 8 12 16 20 24 28 32
    do
       run_file_io
    done

    is_seq=0
    use_direct=0
    use_fsync=0
    for block_size in 4 8 12 16 20 24 28 32
    do
       run_file_io
    done

    is_seq=1
    use_direct=0
    use_fsync=1
    for block_size in 4 8 12 16 20 24 28 32
    do
       run_file_io
    done

    is_seq=0
    use_direct=0
    use_fsync=1
    for block_size in 4 8 12 16 20 24 28 32
    do
       run_file_io
    done

    is_seq=1
    use_direct=1
    use_fsync=0
    for block_size in 4 8 12 16 20 24 28 32
    do
       run_file_io
    done

    is_seq=0
    use_direct=1
    use_fsync=0
    for block_size in 4 8 12 16 20 24 28 32
    do
       run_file_io
    done
}
