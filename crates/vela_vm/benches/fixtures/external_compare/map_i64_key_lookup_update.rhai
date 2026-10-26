
fn run_once() {
    let scores = [0, 3, 8, 2];
    let total = 0;
    for tick in 0..120 {
        scores[1] = scores[1] + tick % 5;
        scores[3] = scores[3] + 1;
        total += scores[1] + scores[2];
    }
    total + scores[3]
}

fn run(iterations) {
    let checksum = 0;
    for iteration in 0..iterations {
        checksum += run_once();
    }
    checksum
}
