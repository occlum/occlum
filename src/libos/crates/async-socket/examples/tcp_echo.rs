include!("common.in");

fn main() {
    init_async_rt();

    async_rt::task::block_on(tcp_echo());
}
