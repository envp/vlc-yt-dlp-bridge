local dialog, link, status
local quality, loaded_id, available
local helper = '"%APPDATA%\\vlc\\lua\\extensions\\vlc-youtube-bridge.exe"'

function descriptor()
    return {
        title = "YouTube Bridge",
        version = "0.1.0",
        shortdesc = "Play YouTube links in VLC",
        capabilities = {}
    }
end

local function video_id(text)
    text = text:match("^%s*(.-)%s*$")
    if #text == 11 and text:match("^[A-Za-z0-9_-]+$") then
        return text
    end

    local host, path = text:match("^https?://([^/]+)(/[^#]*)")
    if not host then return nil end
    host = host:lower()
    local id
    if host == "youtu.be" then
        id = path:match("^/([A-Za-z0-9_-]+)$") or path:match("^/([A-Za-z0-9_-]+)%?")
    elseif host == "www.youtube.com" or host == "youtube.com"
        or host == "music.youtube.com" or host == "m.youtube.com" then
        local route, query = path:match("^([^?]+)%?(.*)$")
        if route == "/watch" then
            id = ("&" .. query .. "&"):match("&v=([A-Za-z0-9_-]+)&")
        else
            local video_path = route or path
            local kind, candidate = video_path:match("^/([a-z]+)/([A-Za-z0-9_-]+)$")
            if kind == "shorts" or kind == "live" or kind == "embed" then
                id = candidate
            end
        end
    end
    if id and #id == 11 then return id end
end

local function invoke(args)
    -- Only fixed words and an allowlisted video ID can reach this command.
    local pipe = io.popen("if exist " .. helper .. " (" .. helper .. " " .. args .. ")", "r")
    if not pipe then return nil end
    local output = pipe:read("*a")
    pipe:close()
    local lines = {}
    for line in output:gmatch("[^\r\n]+") do
        lines[#lines + 1] = line
    end
    return lines[1], lines[2], lines
end

local function load_qualities()
    local id = video_id(link:get_text())
    if not id then
        status:set_text("Paste a YouTube video or track link first.")
        return
    end
    loaded_id, available = nil, {}
    status:set_text("Loading qualities...")
    dialog:update()
    vlc.keep_alive()
    local first, _, lines = invoke("qualities " .. id)
    if not first or not first:match("^%d+$") then
        status:set_text(first or "Could not start the bridge helper.")
        return
    end
    local labels = {}
    for _, value in ipairs(lines) do
        if value:match("^%d+$") then
            available[value] = true
            labels[#labels + 1] = value .. "p"
        end
    end
    loaded_id = id
    quality:set_text(first)
    status:set_text("Available: " .. table.concat(labels, ", "))
end

local function play(mode)
    local id = video_id(link:get_text())
    if not id then
        status:set_text("Paste a YouTube video or track link first.")
        return
    end

    if mode == "video" then
        local height = quality:get_text():match("^%s*(.-)%s*$"):lower():gsub("p$", "")
        if height ~= "" and height ~= "auto" then
            if loaded_id ~= id then
                status:set_text("Click Load qualities for the new link.")
                return
            end
            if not available[height] then
                status:set_text("Enter one of the available resolutions, or clear the field for automatic quality.")
                return
            end
            mode = height
        end
    end

    status:set_text("Looking up the stream...")
    dialog:update()
    vlc.keep_alive()
    local result, audio = invoke("play " .. id .. " " .. mode)
    if not result or not result:match("^https://") then
        status:set_text(result or "Bridge helper missing. Run install.ps1, then restart VLC.")
        return
    end

    local options = mode == "audio" and {":no-video"} or {}
    if audio and audio:match("^https://") then
        options[#options + 1] = ":input-slave=" .. audio
    end
    local added = vlc.playlist.add({{path = result, name = "YouTube " .. id, options = options}})
    status:set_text(added == 1 and "Opened in VLC." or "VLC could not open the stream.")
end

local function browser()
    local id = video_id(link:get_text())
    local result = invoke(id and ("browser " .. id) or "browser")
    status:set_text(result or "Bridge helper missing. Run install.ps1, then restart VLC.")
end

function activate()
    loaded_id = nil
    dialog = vlc.dialog("YouTube Bridge")
    dialog:add_label("YouTube video or track link", 1, 1, 4, 1)
    link = dialog:add_text_input("", 1, 2, 4, 1, 480)
    dialog:add_button("Load qualities", load_qualities, 1, 3)
    dialog:add_label("Resolution (blank for automatic)", 2, 3, 2, 1)
    quality = dialog:add_text_input("", 4, 3)
    dialog:add_button("Play video", function() play("video") end, 1, 4)
    dialog:add_button("Compatible video", function() play("compatible") end, 2, 4)
    dialog:add_button("Listen", function() play("audio") end, 3, 4)
    dialog:add_button("Open YouTube / Sign in", browser, 4, 4)
    status = dialog:add_label("VLC playback uses no Google account.", 1, 5, 4, 1)
    dialog:show()
end

function deactivate()
    if dialog then
        dialog:delete()
        dialog = nil
    end
end

function close()
    vlc.deactivate()
end
