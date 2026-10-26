
function run_once()
    local total = 0
    for tick = 0, 95 do
        local active = {quest = true, raid = true}
        active.event = true
        active.quest = true
        active.raid = nil
        if active.quest and active.event and not active.raid then
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
