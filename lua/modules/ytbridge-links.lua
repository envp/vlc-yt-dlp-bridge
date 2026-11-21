local function valid_id(text)
    return text and #text == 11 and text:match("^[A-Za-z0-9_-]+$")
end

local function parse(text)
    text = text:match("^%s*(.-)%s*$")
    if valid_id(text) then return text, nil, false end

    local host, path = text:match("^https?://([^/]+)(/[^#]*)")
    if not host then return nil end
    host = host:lower()
    local music = host == "music.youtube.com"
    if not music and host ~= "youtu.be" and host ~= "youtube.com"
        and host ~= "www.youtube.com" and host ~= "m.youtube.com" then
        return nil
    end

    local route = path:match("^[^?]+")
    local query = "&" .. (path:match("%?(.*)$") or "") .. "&"
    if route == "/playlist" and host ~= "youtu.be" then
        local list = query:match("&list=([A-Za-z0-9_-]+)&")
        if list and #list >= 2 and #list <= 150 then
            return nil, list, music
        end
        return nil
    end

    local id
    if host == "youtu.be" then
        id = route:match("^/([A-Za-z0-9_-]+)$")
    elseif route == "/watch" then
        id = query:match("&v=([A-Za-z0-9_-]+)&")
    else
        local kind, candidate = route:match("^/([a-z]+)/([A-Za-z0-9_-]+)$")
        if kind == "shorts" or kind == "live" or kind == "embed" then id = candidate end
    end
    if valid_id(id) then return id, nil, music end
end

local function playback(id, list, mode)
    if list then
        return "ytbridge://playlist/" .. mode .. "/" .. list, "YouTube playlist"
    end
    if id then
        return "ytbridge://play/" .. id .. "/" .. mode, "YouTube " .. id
    end
end

return {parse = parse, playback = playback}
