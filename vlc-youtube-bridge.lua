local dialog, link, status
local links

function descriptor()
    return {
        title = "YouTube Bridge",
        version = "0.2.0",
        shortdesc = "Play YouTube links in VLC",
        capabilities = {}
    }
end

local function query(request)
    local stream = vlc.stream("ytbridge://query/" .. request)
    if stream then return stream:read(4096) end
end

local missing_module = "Native module unavailable. Install the plugin DLL and restart VLC."

local function play(audio_only)
    local id, list = links.parse(link:get_text())
    local path, name = links.playback(id, list, audio_only and "audio" or "video")
    if not path then
        status:set_text("Paste a valid YouTube link or video ID.")
        return
    end
    if not query("ping") then
        status:set_text(missing_module)
        return
    end

    if vlc.playlist.add({{path = path, name = name}}) == 1 then
        vlc.deactivate()
    else
        status:set_text("VLC could not queue the request.")
    end
end

function activate()
    local ok, parser = pcall(require, "ytbridge-links")
    links = ok and parser or nil
    dialog = vlc.dialog("YouTube Bridge")
    if not links then
        dialog:add_label("Bridge files are missing. Reinstall the plugin and restart VLC.")
        dialog:show()
        return
    end
    dialog:add_label("YouTube video, track, or playlist link", 1, 1, 2, 1)
    link = dialog:add_text_input("", 1, 2, 2, 1, 420)
    dialog:add_button("Play video", function() play(false) end, 1, 3)
    dialog:add_button("Listen", function() play(true) end, 2, 3)
    status = dialog:add_label("", 1, 4, 2, 1)
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
