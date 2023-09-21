fn main() -> Result<(), Box<dyn std::error::Error>> {
    a::test_cpp_ffi();
    b::test_multithread_mmap()
}

mod a {
    extern crate libc;

    extern "C" {
        fn increment_by_one(input: *mut libc::c_int);
    }

    pub fn test_cpp_ffi() {
        let mut input = 5;
        let old = input;
        unsafe { increment_by_one(&mut input) };
        assert_eq!(old + 1, input);
        println!("Cpp FFI test passed!");
    }
}

mod b {
    use std::fs::File;
    use std::io::prelude::*;
    use std::io::{Cursor, Read};
    use std::thread::{self, JoinHandle};

    pub fn test_multithread_mmap() -> Result<(), Box<dyn std::error::Error>> {
        let mut file = File::create("/tmp/file.txt")?;
        writeln!(
            file,
            "e785a7d529d589f13e610548b54ac636e30ff4c4e4d834b903b460"
        )?;

        for _ in 0..1000 {
            // several thread reads same file
            let handlers = (1..4)
                .map(|_| {
                    thread::spawn(|| -> Result<_, std::io::Error> {
                        let file = File::open("/tmp/file.txt").unwrap();
                        let mmap = unsafe { memmap::Mmap::map(&file).unwrap() };
                        let mut cursor = Cursor::new(mmap.as_ref());
                        let mut buffer: [u8; 6] = [0; 6];
                        cursor.read_exact(&mut buffer)?;
                        Ok(buffer)
                    })
                })
                .collect::<Vec<JoinHandle<Result<_, _>>>>();

            for handler in handlers {
                match handler.join().unwrap() {
                    Ok(data) => assert_eq!(b"e785a7", &data),
                    Err(e) => panic!("Error: {:?}", e),
                }
            }
        }
        println!("Multithreading mmap test passed!");
        Ok(())
    }
}
