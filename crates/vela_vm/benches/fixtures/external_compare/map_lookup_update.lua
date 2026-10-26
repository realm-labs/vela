
function run_once()
    local scores = {quest = 3, raid = 8, daily = 2}
    local total = 0
    for tick = 0, 119 do
        scores.quest = scores.quest + tick % 5
        scores.daily = (scores.daily or 0) + 1
        total = total + (scores.quest or 0) + (scores.raid or 0)
    end
    return total + (scores.daily or 0)
end

function run(iterations)
    local checksum = 0
    for _ = 1, iterations do
        checksum = checksum + run_once()
    end
    return checksum
end
