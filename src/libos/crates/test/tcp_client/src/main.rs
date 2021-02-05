use std::process;
use std::time::Instant;

fn main() {
    let pid = process::id();
    println!("[client {}] started...", pid);

    let socket_fd = unsafe {
        libc::socket(
            libc::AF_INET,
            libc::SOCK_STREAM | libc::SOCK_CLOEXEC,
            libc::IPPROTO_TCP,
        )
    };
    if socket_fd < 0 {
        println!("[client {}] create socket failed, ret: {}", pid, socket_fd);
        return;
    }

    let val: i32 = 1;
    let ret = unsafe {
        libc::setsockopt(
            socket_fd,
            libc::IPPROTO_TCP,
            libc::TCP_NODELAY,
            &val as *const i32 as _,
            core::mem::size_of::<i32>() as u32,
        )
    };
    assert!(ret != -1);

    let servaddr = libc::sockaddr_in {
        sin_family: libc::AF_INET as u16,
        sin_port: 3456_u16.to_be(),
        sin_addr: libc::in_addr { s_addr: 0 },
        sin_zero: [0; 8],
    };
    let ret = unsafe {
        libc::connect(
            socket_fd,
            &servaddr as *const _ as *const libc::sockaddr,
            core::mem::size_of::<libc::sockaddr_in>() as u32,
        )
    };
    if ret < 0 {
        println!("[client {}] connect failed, ret: {}", pid, ret);
        unsafe {
            libc::close(socket_fd);
        }
        return;
    }
    println!("[client {}] connected!", pid);

    let mut buf = vec![0u8; 2048];
    let mut cnt = 0;
    let start = Instant::now();
    loop {
        let mut ret =
            unsafe { libc::write(socket_fd, buf.as_ptr() as *const libc::c_void, buf.len()) };
        if ret < 0 {
            println!("[client {}] write failed, ret: {}, cnt: {}", pid, ret, cnt);
            unsafe {
                libc::close(socket_fd);
            }
            return;
        }

        if ret < buf.len() as isize {
            println!(
                "[client {}] write the rest buffer... {}...{}, iter: {}",
                pid,
                ret,
                buf.len(),
                cnt
            );
            let mut cur_ptr = unsafe { buf.as_ptr().add(ret as usize) };
            let mut cur_len = buf.len() - ret as usize;
            while cur_len > 0 {
                let next_ret =
                    unsafe { libc::write(socket_fd, cur_ptr as *const libc::c_void, cur_len) };
                if next_ret < 0 {
                    println!("[client {}] write failed, ret: {}", pid, ret);
                    unsafe {
                        libc::close(socket_fd);
                    }
                    return;
                }
                cur_ptr = unsafe { cur_ptr.add(next_ret as usize) };
                cur_len -= next_ret as usize;
            }
        }

        ret = unsafe { libc::read(socket_fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
        if ret < 0 {
            println!("[client {}] read failed, ret: {}, cnt: {}", pid, ret, cnt);
            unsafe {
                libc::close(socket_fd);
            }
            return;
        }

        if ret < buf.len() as isize {
            println!(
                "[client {}] read the rest buffer... {}...{}, iter：{}",
                pid,
                ret,
                buf.len(),
                cnt
            );
            let mut cur_ptr = unsafe { buf.as_mut_ptr().add(ret as usize) };
            let mut cur_len = buf.len() - ret as usize;
            while cur_len > 0 {
                let next_ret =
                    unsafe { libc::read(socket_fd, cur_ptr as *mut libc::c_void, cur_len) };
                if next_ret < 0 {
                    println!("[client {}] read failed, ret: {}", pid, ret);
                    unsafe {
                        libc::close(socket_fd);
                    }
                    return;
                }
                cur_ptr = unsafe { cur_ptr.add(next_ret as usize) };
                cur_len -= next_ret as usize;
            }
        }

        cnt += 1;
        if cnt % 100000 == 0 {
            let duration = start.elapsed();
            println!("[client {}] Time is {:?}, iters: {}", pid, duration, cnt);
        }
        if cnt == 1000000 {
            break;
        }
    }

    println!("[client {}] close and exit", pid);
    unsafe {
        libc::close(socket_fd);
    }
}
