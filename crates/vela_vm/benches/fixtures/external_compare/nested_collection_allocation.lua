
function run_once()
    local total = 0
    for tick = 0, 63 do
        local rows = {
            {tick, tick + 1, tick + 2},
            {tick + 3, tick + 4, tick + 5},
            {tick + 6, tick + 7, tick + 8},
        }
        local labels = {
            alpha = tick,
            beta = tick + 2,
            gamma = tick + 4,
        }
        total = total + rows[1][2] + rows[2][3] + rows[3][1]
        total = total + labels["alpha"] + labels["gamma"]
    end
    return total
end

function run(iterations)
    local checksum = 0
    for _ = 1, iterations do
        checksum = checksum + run_once()
    end
    return checksum
end
