
function run_once()
    local total = 0
    for tick = 0, 95 do
        local alice = {id = 1, level = 10}
        local bob = {id = 2, level = 20}
        local alice_copy = {id = 1, level = 10}
        local active = {}
        active[alice] = true
        active[bob] = true
        active[alice] = true
        active[bob] = nil
        alice.level = alice.level + 1
        if active[alice] and not active[bob] and not active[alice_copy] then
            local len = 0
            for _ in pairs(active) do
                len = len + 1
            end
            total = total + len + alice.level % 7 + tick % 5
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
