
fn run_once() {
    let total = 0;
    for tick in 0..72 {
        let text = "alpha,beta,gamma,delta";
        let parts = text.split(",");
        let joined = parts[0] + ":" + parts[1] + ":" + parts[2] + ":" + parts[3];
        total += joined.len() + 10 + tick % 11;
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
