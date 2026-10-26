
function run_once()
    local active = {}
    for value = 0, 63 do
        active[value] = true
    end
    local total = 0
    for tick = 0, 191 do
        active[64] = true
        active[64] = nil
        if active[0] and active[63] and not active[128] then
            local len = 0
            for _ in pairs(active) do
                len = len + 1
            end
            total = total + len + tick % 7
        end
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
