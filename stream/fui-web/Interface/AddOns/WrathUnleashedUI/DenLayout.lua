-- DenLayout (3.3.5a API): one place that says where the Den's windows go, so they never sit on each other (owner,
-- 5 Nov, live in Your Den: "the ui is a mess too": the Dojo drawn over the play corner, its title over its own Train
-- tab, the hint line under the status pill, a bags line bleeding in). Laid out as his LANDSCAPE editor: a top
-- toolbar, a slim tool rail down the left, the gallery docked bottom left, one inspector on the right for what you
-- are working on, one status line at the bottom, the mode's banner and the invite card at the top middle.
--
-- The slots (UI units, the screen's bottom left at 0,0; GetScreenWidth / Height are UIParent's size, so the game's
-- UI scale is in them already):
--   top        the strip (DenBar.lua), centred, clear of the player frame and the minimap
--   left       the tool rail, or play mode's corner (DenBar.lua): under the player frame, above the chat
--   gallery    the gallery (Builder.lua), right of the rail, above the chat (it finds its own place round the chat)
--   inspector  the right-hand panel: the selection (Builder.lua), the Dojo, the Arena, Den home, Tools: left of the
--              minimap and the side bars, under the strip, over the status line; one at a time
--   status     the one status line (Builder.lua's pill): bottom, right of the chat, left of the side bars
--   toast      the top middle under the strip: the mode's banner, the save / invite card (stacked, never on each other)
--   centre     a big window (Files, the guide, Teleports): the middle, under the toasts
-- Opening a window in a taken slot puts the other one away until it closes (it comes back by itself); a window that
-- would cover another slot's window does the same to that one. The strip, the rail and the status line are never put
-- away. A window taller or wider than its slot is scaled down to fit (never under 0.7).
--
--   L.Dock(frame, slot[, opts])   opts.place = false: the frame places itself (the slot only keeps others off it);
--                                 opts.keep = true: never put away; opts.name: for the den-test's report
--   L.Area(slot)                  { l, b, r, t } for that slot now
--   L.Layout()                    every docked window in place again (a resize, a new UI scale)
--   L.Overlaps()                  "a/b" for docked windows on each other (the den-test's report, the layout test)
local W = WrathUnleashedUI
local L = { docked = {}, order = {}, clock = 0 }
W.DenLayout = L

-- the game's own frames at their stock places (UI units), as Builder.lua's LAYOUT keeps clear of them
L.PLAYER_BOTTOM = 118   -- under the player frame (104 high at the top left)
L.CHAT_TOP = 232        -- above the chat (its top is 215 at the stock place)
L.CHAT_RIGHT = 470      -- right of the chat (its right edge is 462)
L.MINIMAP_W = 198       -- left of the minimap (192) and of the two side bars (87)
L.SIDE_W = 96           -- the side bars alone (bottom right, from 98 up)
L.BAR_TOP, L.BAR_H = 6, 40
L.STATUS_BOTTOM, L.STATUS_H = 152, 30
L.GAP = 8
L.MIN_FIT = 0.7

local function screen()
    local w = (GetScreenWidth and tonumber((GetScreenWidth()))) or 1366
    local h = (GetScreenHeight and tonumber((GetScreenHeight()))) or 768
    return w, h
end

-- Where each slot is now.
function L.Area(slot)
    local sw, sh = screen()
    local under = sh - L.BAR_TOP - L.BAR_H - L.GAP          -- under the strip
    if slot == "top" then return { l = 222, b = sh - L.BAR_TOP - L.BAR_H, r = sw - 200, t = sh - L.BAR_TOP } end
    if slot == "left" then return { l = 8, b = L.CHAT_TOP, r = L.CHAT_RIGHT, t = sh - L.PLAYER_BOTTOM } end
    if slot == "gallery" then return { l = 80, b = L.CHAT_TOP, r = sw - L.MINIMAP_W - L.GAP, t = sh - 112 } end
    if slot == "inspector" then
        return { l = math.floor(sw / 2), b = L.STATUS_BOTTOM + L.STATUS_H + L.GAP, r = sw - L.MINIMAP_W, t = under }
    end
    if slot == "status" then return { l = L.CHAT_RIGHT, b = L.STATUS_BOTTOM, r = sw - L.SIDE_W, t = L.STATUS_BOTTOM + L.STATUS_H } end
    -- the top middle and the centre: between the rail and the inspector when one is open
    local right = sw - 200
    local ins = L.SlotRect("inspector")
    if ins then right = math.min(right, ins.l - L.GAP) end
    -- and right of the player frame, or of play mode's corner when it is wider (owner's dojo-overlap.png)
    local left = 222
    local lft = L.SlotRect("left")
    if lft then left = math.max(left, lft.r + L.GAP) end
    if slot == "toast" then return { l = left, b = math.floor(sh * 0.55), r = math.max(right, left + 480), t = under } end
    if slot == "centre" then
        local top = under - L.StackHeight("toast")
        return { l = left, b = L.STATUS_BOTTOM + L.STATUS_H + L.GAP, r = math.max(right, left + 480), t = top }
    end
    return { l = 0, b = 0, r = sw, t = sh }
end

-- ---------------------------------------------------------------- a frame's rectangle (UIParent units)
local function num(v) return tonumber(v) end
function L.Rect(f)
    if not (f and f.IsVisible and f:IsVisible() and f.GetLeft) then return nil end
    local l, b, w, h = num((f:GetLeft())), num((f:GetBottom())), num((f:GetWidth())), num((f:GetHeight()))
    if not (l and b and w and h) then return nil end
    local s = 1
    if f.GetEffectiveScale and UIParent and UIParent.GetEffectiveScale then
        local fe, ue = num((f:GetEffectiveScale())), num((UIParent:GetEffectiveScale()))
        if fe and ue and ue > 0 then s = fe / ue end
    end
    return { l = l * s, b = b * s, r = (l + w) * s, t = (b + h) * s }
end
local function hit(a, c, pad)
    pad = pad or 2
    return a and c and math.min(a.r, c.r) - math.max(a.l, c.l) > pad and math.min(a.t, c.t) - math.max(a.b, c.b) > pad
end
L.Hit = hit

-- the rectangle of the window shown in a slot now (nil: none)
function L.SlotRect(slot)
    for _, d in ipairs(L.order) do
        if d.slot == slot and d.frame:IsShown() then local r = L.Rect(d.frame); if r then return r end end
    end
end

-- ---------------------------------------------------------------- docking
local busy
function L.Dock(frame, slot, opts)
    if not frame then return end
    local d = L.docked[frame]
    if d then d.slot = slot; return d end
    opts = opts or {}
    d = { frame = frame, slot = slot, place = opts.place ~= false, keep = opts.keep == true,
        name = opts.name or (frame.GetName and frame:GetName()) or slot, at = 0 }
    L.docked[frame] = d
    L.order[#L.order + 1] = d
    if frame.HookScript then
        frame:HookScript("OnShow", function() L.Shown(frame) end)
        frame:HookScript("OnHide", function() L.Hidden(frame) end)
    end
    -- the owner hiding it (its x, Esc, its button) while it waits: it stays closed
    if hooksecurefunc and frame.Hide then
        pcall(hooksecurefunc, frame, "Hide", function(self) if not busy then local e = L.docked[self]; if e then e.waiting = nil end end end)
    end
    if frame.IsShown and frame:IsShown() then L.Shown(frame) end
    return d
end
function L.Slot(frame) local d = L.docked[frame]; return d and d.slot end
function L.Waiting(frame) local d = L.docked[frame]; return d and d.waiting ~= nil end

-- the toasts' stack: how tall it is now (the centre slot starts under it)
function L.StackHeight(slot)
    local h = 0
    for _, d in ipairs(L.order) do
        if d.slot == slot and d.frame:IsShown() and (d.frame.GetAlpha == nil or (tonumber((d.frame:GetAlpha())) or 1) > 0.05) then
            h = h + (tonumber((d.frame:GetHeight())) or 0) * (tonumber((d.frame:GetScale())) or 1) + L.GAP
        end
    end
    return h
end

-- One window in its slot: anchored to the slot's corner, scaled down when it doesn't fit.
local CORNER = { inspector = "TOPRIGHT", left = "TOPLEFT", gallery = "BOTTOMLEFT", top = "TOP", status = "BOTTOM", toast = "TOP", centre = "TOP" }
function L.Place(d, y)
    local f = d.frame
    local a = L.Area(d.slot)
    local w, h = tonumber((f:GetWidth())) or 0, tonumber((f:GetHeight())) or 0
    if w <= 0 or h <= 0 then return end
    local top = y or a.t
    local fit = math.min(1, (a.r - a.l) / w, (top - a.b) / h)
    fit = math.max(L.MIN_FIT, fit)
    d.fit = fit
    local parent = (f.GetParent and f:GetParent()) or UIParent
    -- on the world frame (edit mode hides UIParent: Builder.lua B.Dock) it keeps UIParent's size
    local base = 1
    if parent ~= UIParent and parent.GetEffectiveScale and UIParent and UIParent.GetEffectiveScale then
        local ue, pe = tonumber((UIParent:GetEffectiveScale())), tonumber((parent:GetEffectiveScale()))
        if ue and pe and pe > 0 then base = ue / pe end
    end
    if f.SetScale then f:SetScale(base * fit) end
    local p = CORNER[d.slot] or "CENTER"
    -- offsets in the frame's own units (UI units / its fit), from the screen's corner (UIParent covers the screen
    -- also while edit mode hides it)
    local rel = UIParent
    f:ClearAllPoints()
    if p == "TOPRIGHT" then f:SetPoint("TOPRIGHT", rel, "BOTTOMLEFT", a.r / fit, top / fit)
    elseif p == "TOPLEFT" then f:SetPoint("TOPLEFT", rel, "BOTTOMLEFT", a.l / fit, top / fit)
    elseif p == "BOTTOMLEFT" then f:SetPoint("BOTTOMLEFT", rel, "BOTTOMLEFT", a.l / fit, a.b / fit)
    elseif p == "BOTTOM" then f:SetPoint("BOTTOM", rel, "BOTTOMLEFT", (a.l + a.r) / 2 / fit, a.b / fit)
    else f:SetPoint("TOP", rel, "BOTTOMLEFT", (a.l + a.r) / 2 / fit, top / fit) end
end

-- Everything in place, then nothing on anything: the newest window wins, what it covers waits for it.
function L.Layout()
    if busy then return end
    busy = true
    local sw, sh = screen()
    local under = sh - L.BAR_TOP - L.BAR_H - L.GAP
    local y = under
    for _, d in ipairs(L.order) do
        if d.place and d.frame:IsShown() then
            if d.slot == "toast" then
                L.Place(d, y)
                y = y - (tonumber((d.frame:GetHeight())) or 0) * (d.fit or 1) - L.GAP
            elseif d.slot ~= "toast" then
                L.Place(d)
            end
        end
    end
    -- newest first: each one puts away the older windows it covers (never the strip, the rail, the status line)
    local shown = {}
    for _, d in ipairs(L.order) do if d.frame:IsShown() then shown[#shown + 1] = d end end
    table.sort(shown, function(a, b) return a.at > b.at end)
    for i, d in ipairs(shown) do
        if d.frame:IsShown() then
            local r = L.Rect(d.frame)
            for j = i + 1, #shown do
                local o = shown[j]
                if o.frame:IsShown() and not o.keep and (o.slot == d.slot and o.slot ~= "toast" or hit(r, L.Rect(o.frame))) then
                    o.waiting = d.frame
                    o.frame:Hide()
                end
            end
        end
    end
    busy = false
end

function L.Shown(frame)
    local d = L.docked[frame]
    if not d or busy then return end
    L.clock = L.clock + 1
    d.at = L.clock
    d.waiting = nil
    L.Layout()
end

-- A window closed: what waited for it comes back (unless something else still covers it).
function L.Hidden(frame)
    local d = L.docked[frame]
    if not d or busy then return end
    local back = {}
    for _, o in ipairs(L.order) do
        if o.waiting == frame then o.waiting = nil; back[#back + 1] = o end
    end
    if #back == 0 then if d.slot == "toast" then L.Layout() end return end
    busy = true
    for _, o in ipairs(back) do o.frame:Show() end
    busy = false
    L.Layout()
end

-- Docked windows on each other ("a/b"), for the den-test's report and the layout test.
function L.Overlaps()
    local out, shown = {}, {}
    for _, d in ipairs(L.order) do if d.frame:IsVisible() then shown[#shown + 1] = d end end
    for i = 1, #shown do
        for j = i + 1, #shown do
            if hit(L.Rect(shown[i].frame), L.Rect(shown[j].frame)) then out[#out + 1] = shown[i].name .. "/" .. shown[j].name end
        end
    end
    return out
end

-- ---------------------------------------------------------------- text that fits
-- A label column as wide as its longest label (measured), so the buttons after it start clear of every label.
function L.Column(labels, min, pad)
    local w = min or 0
    for _, fs in ipairs(labels) do
        local tw = fs and fs.GetStringWidth and tonumber((fs:GetStringWidth())) or 0
        if tw > w then w = tw end
    end
    return math.ceil(w) + (pad or 12)
end

-- One line in a fixed width: the game ends it in "..." when it is longer (the whole text stays the FontString's).
function L.OneLine(fs, w, h)
    if not fs then return fs end
    if w then fs:SetWidth(w) end
    fs:SetHeight(h or 16)
    if fs.SetWordWrap then fs:SetWordWrap(false) end
    if fs.SetNonSpaceWrap then fs:SetNonSpaceWrap(false) end
    return fs
end

return L
