
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
    let active = [];
    for value in 0..64 {
        active.push(value);
    }
    let total = 0;
    for tick in 0..192 {
        active = add(active, 64);
        active = remove(active, 64);
        if has(active, 0) && has(active, 63) && !has(active, 128) {
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
