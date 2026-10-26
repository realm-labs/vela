
function bump(counter, extra)
    return counter.value + counter.step + extra
end

function run_once()
    local total = 0
    for tick = 0, 95 do
        local counter = { value = tick, step = tick % 5 + 1 }
        total = total + bump(counter, tick % 3)
        total = total + counter.value + counter.step
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
