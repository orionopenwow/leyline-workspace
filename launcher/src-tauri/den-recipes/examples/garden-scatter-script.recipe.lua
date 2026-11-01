-- Garden scatter: a Leyline Den recipe (docs/DEN.md, "Building brain"). A small script, run only on the player's PC
-- by the launcher, in a sandbox (math, string and table only; no files, no network; a memory and a time limit).
-- It returns pieces like a template does: { key = <catalog key>, x, y, z, yaw, scale }, yards from the room's middle.
-- rand(a, b) is a steady random number in [0, 1): the same a, b and seed always give the same number, so the
-- approval card shows exactly what is placed.

recipe = {
    name = "garden-scatter",
    title = "Garden scatter",
    about = "Flowers, plants and bushes scattered over a round bed, never two too close, with a low fence around it.",
    params = {
        x = { default = 0, min = -40, max = 40, about = "the bed's middle: yards from the room's middle" },
        y = { default = 0, min = -40, max = 40 },
        radius = { default = 5, min = 2, max = 20, about = "yards" },
        count = { default = 20, min = 3, max = 120, whole = true, about = "how many plants" },
        gap = { default = 1.2, min = 0.5, max = 5, about = "the least yards between two plants" },
        fence = { default = 1, min = 0, max = 1, whole = true, about = "1: a low fence around the bed" },
        seed = { default = 1, min = 1, max = 99999, whole = true, about = "another number, another scatter" },
    },
}

local PLANTS = { "flower", "fragrantflower", "plant", "bogbeanplant", "concealingbush" }

function build(p)
    local out, tries = {}, 0
    while #out < p.count and tries < p.count * 30 do
        tries = tries + 1
        local a = rand(tries, 1) * 2 * math.pi
        local r = p.radius * math.sqrt(rand(tries, 2))
        local px, py = p.x + r * math.cos(a), p.y + r * math.sin(a)
        local free = true
        for _, q in ipairs(out) do
            if (q.x - px) ^ 2 + (q.y - py) ^ 2 < p.gap ^ 2 then free = false; break end
        end
        if free then
            out[#out + 1] = { key = PLANTS[math.floor(rand(tries, 3) * #PLANTS) + 1], x = px, y = py,
                yaw = rand(tries, 4) * 360, scale = 0.8 + rand(tries, 5) * 0.5 }
        end
    end
    if p.fence == 1 then
        local fr = p.radius + 1
        local n = math.max(6, math.floor(2 * math.pi * fr / 3))
        for i = 0, n - 1 do
            local a = 2 * math.pi * i / n
            out[#out + 1] = { key = "woodfence2", x = p.x + fr * math.cos(a), y = p.y + fr * math.sin(a), yaw = math.deg(a) + 90 }
        end
    end
    return out
end
