include!("common/tcp_echo.rs");

fn main() {
    use std::env;
    let args: Vec<String> = env::args().collect();
    let port: u16 = if args.len() > 1 {
        args[1].parse().unwrap()
    } else {
        3456
    };

    let parallelism: u32 = 1;

    init_async_rt(parallelism);

    async_rt::task::block_on(tcp_echo(port));
}
