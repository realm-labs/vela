
local values = {3, 1, 4, 1, 5, 9, 2, 6}
function run_once()
    local total = 0
    for tick = 0, 199 do
        for _, value in ipairs(values) do
            if value % 2 == 0 then
                total = total + (value * tick) % 17
            else
                total = total + value + tick % 5
            end
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
