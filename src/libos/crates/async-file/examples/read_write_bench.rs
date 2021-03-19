include!("common/bench.rs");

fn main() {
    use std::env;
    let args: Vec<String> = env::args().collect();
    let kb_size = 1024;
    let mb_size = kb_size * kb_size;
    let mut file_num: usize = 1;
    let mut file_block_size: usize = 4 * kb_size;
    let mut file_total_size: usize = 100 * mb_size;
    let mut is_read: bool = true;
    let mut is_seq: bool = true;
    let mut use_fsync: bool = false;
    let mut use_direct: bool = false;
    let mut loops: usize = 100;
    if args.len() > 8 {
        file_num = args[1].parse().unwrap();
        file_block_size = args[2].parse::<usize>().unwrap() * kb_size;
        file_total_size = args[3].parse::<usize>().unwrap() * mb_size;
        is_read = args[4].parse().unwrap();
        is_seq = args[5].parse().unwrap();
        use_fsync = args[6].parse().unwrap();
        use_direct = args[7].parse().unwrap();
        loops = args[8].parse().unwrap();
    }

    init_async_rt();

    read_write_bench(
        file_num,
        file_block_size,
        file_total_size,
        is_read,
        is_seq,
        use_fsync,
        use_direct,
        loops,
    );
}
