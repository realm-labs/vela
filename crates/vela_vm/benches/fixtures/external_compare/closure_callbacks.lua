
function run_once()
    local total = 0
    for tick = 0, 119 do
        local function add(value)
            return value + tick
        end
        local function mix(left, right)
            return left * 3 + right + tick - tick
        end
        total = total + add(tick)
        total = total + add(total % 17)
        total = total + mix(tick, total % 23)
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
