
function run_once()
    local total = 0
    for tick = 0, 63 do
        local values = {7, 3, 11, 5, 2, 13, 17, 19}
        local filtered = {}
        for _, value in ipairs(values) do
            local mapped = value * 2 + tick % 5
            if mapped % 3 ~= 0 then
                filtered[#filtered + 1] = mapped
            end
        end
        table.sort(filtered)
        local sum = 0
        for _, value in ipairs(filtered) do
            sum = sum + value
        end
        total = total + sum + filtered[1] + filtered[#filtered]
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
