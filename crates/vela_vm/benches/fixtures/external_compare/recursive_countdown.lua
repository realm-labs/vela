
function descend(value)
    if value <= 0 then
        return 0
    end
    return value + descend(value - 1)
end

function run_once()
    local total = 0
    for value = 16, 23 do
        total = total + descend(value)
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
