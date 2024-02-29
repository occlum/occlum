fn first_example() {
    use vdso_time::ClockId;

    let time = vdso_time::clock_gettime(ClockId::CLOCK_MONOTONIC).unwrap();
    println!("vdso_time::clock_gettime: {:?}", time);

    let res = vdso_time::clock_getres(ClockId::CLOCK_MONOTONIC).unwrap();
    println!("vdso_time::clock_getres: {:?}", res);
}

fn second_example() {
    use vdso_time::{Vdso, ClockId};

    let vdso = Vdso::new().unwrap();

    let time = vdso.clock_gettime(ClockId::CLOCK_MONOTONIC).unwrap();
    println!("vdso.clock_gettime: {:?}", time);

    let res = vdso.clock_getres(ClockId::CLOCK_MONOTONIC).unwrap();
    println!("vdso.clock_getres: {:?}", res);
}

fn example() {
    first_example();
    second_example();
}