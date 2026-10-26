
fn has(values, target) {
    for value in values {
        if value == target {
            return true;
        }
    }
    false
}

fn add(values, target) {
    if !has(values, target) {
        values.push(target);
    }
    values
}

fn remove(values, target) {
    let kept = [];
    for value in values {
        if value != target {
            kept.push(value);
        }
    }
    kept
}

fn run_once() {
    let total = 0;
    for tick in 0..96 {
        let active = [1, 2];
        active = add(active, 3);
        active = add(active, 1);
        active = remove(active, 2);
        if has(active, 1) && has(active, 3) && !has(active, 2) {
            total += active.len() + tick % 7;
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
