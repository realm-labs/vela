
fn bump(counter, extra) {
    counter.value + counter.step + extra
}

fn run_once() {
    let total = 0;
    for tick in 0..96 {
        let counter = #{ value: tick, step: tick % 5 + 1 };
        total += bump(counter, tick % 3);
        total += counter.value + counter.step;
    }
    total
}

fn run(iterations) {
    let checksum = 0;
    for iteration in 0..iterations {
        checksum += run_once();
    }
    checksum
}
