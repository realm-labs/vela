
function add_one(value)
    return value + 1
end

function mix_pair(left, right)
    return left * 3 + right
end

function run_once()
    local total = 0
    for tick = 0, 239 do
        total = total + add_one(tick)
        total = total + mix_pair(tick, total % 17)
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
