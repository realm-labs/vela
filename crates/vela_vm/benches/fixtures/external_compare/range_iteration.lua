
function run_once()
    local total = 0
    for outer = 0, 7 do
        for value = 0, 127 do
            total = total + value + outer - outer
        end
    end
    for value = 0, 63 do
        total = total + value
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
