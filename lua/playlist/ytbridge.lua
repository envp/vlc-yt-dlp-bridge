local links

local function dropped_link()
    if vlc.access ~= "http" and vlc.access ~= "https" then return nil end
    links = links or require("ytbridge-links")
    return links.parse(vlc.access .. "://" .. vlc.path)
end

function probe()
    local id, list = dropped_link()
    if not id and not list then return false end
    return vlc.stream("ytbridge://query/ping") ~= nil
end

function parse()
    local id, list, music = dropped_link()
    local path, name = links.playback(id, list, music and "audio" or "video")
    return path and {{path = path, name = name}} or {}
end
