
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
        let active = ["quest", "raid"];
        active = add(active, "event");
        active = add(active, "quest");
        active = remove(active, "raid");
        if has(active, "quest") && has(active, "event") && !has(active, "raid") {
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
