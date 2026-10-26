
fn run_once() {
    let scores = #{
        quest: 3,
        raid: 8,
        daily: 2,
    };
    let total = 0;
    for tick in 0..120 {
        scores["quest"] = scores["quest"] + tick % 5;
        scores["daily"] = scores["daily"] + 1;
        total += scores["quest"] + scores["raid"];
    }
    total + scores["daily"]
}

fn run(iterations) {
    let checksum = 0;
    for iteration in 0..iterations {
        checksum += run_once();
    }
    checksum
}
