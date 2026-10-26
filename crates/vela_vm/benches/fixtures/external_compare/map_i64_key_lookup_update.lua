
function run_once()
    local scores = {[1] = 3, [2] = 8, [3] = 2}
    local total = 0
    for tick = 0, 119 do
        scores[1] = (scores[1] or 0) + tick % 5
        scores[3] = (scores[3] or 0) + 1
        total = total + (scores[1] or 0) + (scores[2] or 0)
    end
    return total + (scores[3] or 0)
end

function run(iterations)
    local checksum = 0
    for _ = 1, iterations do
        checksum = checksum + run_once()
    end
    return checksum
end
