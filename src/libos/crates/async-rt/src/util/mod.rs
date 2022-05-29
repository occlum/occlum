mod atomic_bits;

pub use atomic_bits::AtomicBits;

pub fn align_up(n: usize, a: usize) -> usize {
    debug_assert!(a >= 2 && a.is_power_of_two());
    (n + a - 1) & !(a - 1)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_align_up() {
        let input_ns = [0, 1, 2, 9, 15, 21, 32, 47, 50];
        let input_as = [2, 2, 2, 2, 4, 4, 8, 8, 8];
        let output_ns = [0, 2, 2, 10, 16, 24, 32, 48, 56];

        for i in 0..input_ns.len() {
            let n = input_ns[i];
            let a = input_as[i];
            let n2 = output_ns[i];
            assert!(align_up(n, a) == n2);
        }
    }
}
