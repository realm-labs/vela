
fn run_once() {
    let total = 0;
    for tick in 0..64 {
        let values = [7, 3, 11, 5, 2, 13, 17, 19];
        let mapped = values.map(|value| value * 2 + tick % 5);
        let filtered = mapped.filter(|value| value % 3 != 0);
        filtered.sort();
        let sum = 0;
        for index in 0..filtered.len() {
            sum += filtered[index];
        }
        total += sum + filtered[0] + filtered[filtered.len() - 1];
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
