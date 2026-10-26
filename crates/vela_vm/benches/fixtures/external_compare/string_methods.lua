
function run_once()
    local total = 0
    local labels = {"quest", "raid", "daily", "bonus"}
    for tick = 0, 49 do
        for _, label in ipairs(labels) do
            if string.sub(label, 1, 1) == "q" or string.find(label, "i", 1, true) ~= nil then
                total = total + #label + tick % 7
            else
                total = total + #label
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
