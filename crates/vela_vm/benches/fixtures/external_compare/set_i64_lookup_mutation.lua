
function run_once()
    local total = 0
    for tick = 0, 95 do
        local active = {[1] = true, [2] = true}
        active[3] = true
        active[1] = true
        active[2] = nil
        if active[1] and active[3] and not active[2] then
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
