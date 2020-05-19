extern crate libc;

extern "C" {
    fn increment_by_one(input: *mut libc::c_int);
}

fn main() {
    let mut input = 5;
    let old = input;
    unsafe { increment_by_one(&mut input) };
    println!("{} + 1 = {}", old, input);
}
