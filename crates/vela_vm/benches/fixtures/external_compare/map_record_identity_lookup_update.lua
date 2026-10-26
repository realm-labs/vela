
function run_once()
    local alice = {id = 1, level = 10}
    local bob = {id = 2, level = 20}
    local alice_copy = {id = 1, level = 10}
    local scores = {}
    scores[alice] = 3
    scores[bob] = 8
    local total = 0
    for tick = 0, 95 do
        alice.level = alice.level + 1
        scores[alice] = (scores[alice] or 0) + tick % 5
        if scores[alice] ~= nil and scores[bob] ~= nil and scores[alice_copy] == nil then
            total = total + (scores[alice] or 0) + (scores[bob] or 0) + alice.level % 7
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
