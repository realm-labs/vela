
function run_once()
    local total = 0
    for value = 0, 199 do
        if value % 3 == 0 then
            total = total + value * 2
        elseif value > 180 then
            break
        else
            total = total + (value * 5) % 17
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
