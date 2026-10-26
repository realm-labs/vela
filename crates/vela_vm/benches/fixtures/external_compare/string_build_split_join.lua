
function split_commas(text)
    local parts = {}
    for part in string.gmatch(text, "([^,]+)") do
        parts[#parts + 1] = part
    end
    return parts
end

function run_once()
    local total = 0
    for tick = 0, 71 do
        local text = "alpha,beta,gamma,delta"
        local parts = split_commas(text)
        local joined = table.concat(parts, ":")
        total = total + #joined + 10 + tick % 11
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
