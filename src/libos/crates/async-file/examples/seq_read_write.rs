include!("common.in");

fn main() {
    init_async_rt();

    seq_read_write();
    // libc_seq_read_write();
}
