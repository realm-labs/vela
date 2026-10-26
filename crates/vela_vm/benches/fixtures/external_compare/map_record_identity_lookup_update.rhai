
fn run_once() {
    let alice_level = 10;
    let alice_score = 3;
    let bob_score = 8;
    let total = 0;
    for tick in 0..96 {
        alice_level += 1;
        alice_score += tick % 5;
        total += alice_score + bob_score + alice_level % 7;
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
