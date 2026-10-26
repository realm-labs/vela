
function run_once()
    local total = 0.0
    local value = 1.25
    for tick = 0, 255 do
        total = total + value * 1.5 - value / 3.0 + 0.75
        value = value + 0.5
    end
    return math.floor(total + 0.5)
end

function run(iterations)
    local checksum = 0
    for _ = 1, iterations do
        checksum = checksum + run_once()
    end
    return checksum
end
