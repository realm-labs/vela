
fn run_once() {
    let total = 0;
    let labels = ["quest", "raid", "daily", "bonus"];
    for tick in 0..50 {
        for label in labels {
            if label.starts_with("q") || label.contains("i") {
                total += label.len() + tick % 7;
            } else {
                total += label.len();
            }
        }
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
