#include <winsock2.h>
#include <BaseTsd.h>
typedef SSIZE_T ssize_t;
#define poll WSAPoll
#define strcasecmp _stricmp
#define _CRT_NONSTDC_NO_WARNINGS
#define __PLUGIN__
#define MODULE_STRING "youtube_bridge"
#define MODULE_SYMBOL 3_0_0f
#include <vlc_common.h>
#include <vlc_input_item.h>
#include <vlc_interrupt.h>
#include <vlc_plugin.h>
#include <vlc_stream.h>

#include "youtube_bridge.h"

struct response {
    char *text;
    size_t length;
    size_t offset;
};

static bool starts_with(const char *text, const char *prefix)
{
    return strncmp(text, prefix, strlen(prefix)) == 0;
}

static ssize_t Read(stream_t *stream, void *buffer, size_t length)
{
    struct response *response = stream->p_sys;
    size_t count = response->length - response->offset;
    if (count > length)
        count = length;
    if (buffer != NULL)
        memcpy(buffer, response->text + response->offset, count);
    response->offset += count;
    return (ssize_t)count;
}

static int Control(stream_t *stream, int query, va_list arguments)
{
    switch (query) {
        case STREAM_CAN_SEEK:
        case STREAM_CAN_FASTSEEK:
        case STREAM_CAN_PAUSE:
        case STREAM_CAN_CONTROL_PACE:
            *va_arg(arguments, bool *) = false;
            return VLC_SUCCESS;
        case STREAM_GET_PTS_DELAY:
            *va_arg(arguments, int64_t *) = 0;
            return VLC_SUCCESS;
        case STREAM_IS_DIRECTORY:
            return stream->pf_readdir != NULL ? VLC_SUCCESS : VLC_EGENERIC;
        case STREAM_GET_SIZE:
            if (stream->pf_read == NULL)
                return VLC_EGENERIC;
            *va_arg(arguments, uint64_t *) = ((struct response *)stream->p_sys)->length;
            return VLC_SUCCESS;
        default:
            return VLC_EGENERIC;
    }
}

static int ReadTrack(stream_t *stream, input_item_node_t *node)
{
    const char *request = stream->psz_location;
    bool succeeded;
    char *response = bridge_run(request, &succeeded, vlc_killed);
    if (!succeeded) {
        msg_Warn(stream, "%s", response);
        bridge_free(response);
        return VLC_EGENERIC;
    }

    char *title = response;
    char *artwork = strchr(title, '\n');
    char *source = artwork == NULL ? NULL : strchr(artwork + 1, '\n');
    char *video = source == NULL ? NULL : strchr(source + 1, '\n');
    if (video == NULL) {
        bridge_free(response);
        return VLC_EGENERIC;
    }
    *artwork++ = '\0';
    *source++ = '\0';
    *video++ = '\0';
    char *audio = strchr(video, '\n');
    if (audio != NULL)
        *audio++ = '\0';
    input_item_t *item = input_item_NewStream(video, title, -1);
    if (item == NULL) {
        bridge_free(response);
        return VLC_ENOMEM;
    }
    input_item_SetTitle(item, title);
    input_item_SetURL(item, source);
    if (*artwork != '\0')
        input_item_SetArtworkURL(item, artwork);
    int result = VLC_SUCCESS;
    if (audio != NULL) {
        size_t size = strlen(audio) + sizeof("input-slave=");
        char *option = malloc(size);
        if (option == NULL)
            result = VLC_ENOMEM;
        else {
            snprintf(option, size, "input-slave=%s", audio);
            result = input_item_AddOption(item, option, VLC_INPUT_OPTION_TRUSTED);
            free(option);
        }
    } else if (strlen(request) >= 6 && strcmp(request + strlen(request) - 6, "/audio") == 0) {
        result = input_item_AddOption(item, "no-video", VLC_INPUT_OPTION_TRUSTED);
    }
    if (result == VLC_SUCCESS) {
        if (input_item_node_AppendItem(node, item) == NULL)
            result = VLC_ENOMEM;
        else if (*artwork != '\0')
            libvlc_ArtRequest(stream->obj.libvlc, item, META_REQUEST_OPTION_NONE);
    }
    input_item_Release(item);
    bridge_free(response);
    return result;
}

static int ReadPlaylist(stream_t *stream, input_item_node_t *node)
{
    const char *location = stream->psz_location;
    const char *mode = starts_with(location, "playlist/audio/") ? "audio" : "video";
    char request[sizeof("playlist/") + 150];
    const char *id = strrchr(location, '/') + 1;
    if (strlen(id) > 150)
        return VLC_EGENERIC;
    snprintf(request, sizeof(request), "playlist/%s", id);

    bool succeeded;
    char *entries = bridge_run(request, &succeeded, vlc_killed);
    if (!succeeded) {
        msg_Warn(stream, "%s", entries);
        bridge_free(entries);
        return VLC_EGENERIC;
    }

    int result = VLC_SUCCESS;
    for (char *line = entries; line != NULL;) {
        char *next = strchr(line, '\n');
        if (next != NULL)
            *next++ = '\0';
        char *title = strchr(line, '\t');
        if (title != NULL) {
            *title++ = '\0';
            char url[sizeof("ytbridge://play//video") + 11];
            snprintf(url, sizeof(url), "ytbridge://play/%s/%s", line, mode);
            input_item_t *item = input_item_NewDirectory(url, title, ITEM_NET);
            if (item == NULL) {
                result = VLC_ENOMEM;
                break;
            }
            bool added = input_item_node_AppendItem(node, item) != NULL;
            input_item_Release(item);
            if (!added) {
                result = VLC_ENOMEM;
                break;
            }
        }
        line = next;
    }
    bridge_free(entries);
    return result;
}

static int Open(vlc_object_t *object)
{
    stream_t *stream = (stream_t *)object;
    const char *location = stream->psz_location;
    if (stream->b_preparsing || location == NULL)
        return VLC_EGENERIC;

    stream->pf_control = Control;
    if (starts_with(location, "play/")) {
        stream->pf_readdir = ReadTrack;
    } else if (starts_with(location, "playlist/audio/") || starts_with(location, "playlist/video/")) {
        stream->pf_readdir = ReadPlaylist;
    } else if (starts_with(location, "query/") && stream->p_input == NULL) {
        const char *query = location + sizeof("query/") - 1;
        if (strcmp(query, "ping") != 0 && !starts_with(query, "qualities/"))
            return VLC_EGENERIC;
        struct response *response = calloc(1, sizeof(*response));
        if (response == NULL)
            return VLC_ENOMEM;
        bool succeeded;
        response->text = bridge_run(query, &succeeded, vlc_killed);
        response->length = strlen(response->text);
        stream->p_sys = response;
        stream->pf_read = Read;
    } else {
        return VLC_EGENERIC;
    }
    return VLC_SUCCESS;
}

static void Close(vlc_object_t *object)
{
    stream_t *stream = (stream_t *)object;
    struct response *response = stream->p_sys;
    if (response != NULL) {
        bridge_free(response->text);
        free(response);
    }
}

vlc_module_begin()
    set_shortname("YouTube Bridge")
    set_description("Native YouTube Bridge")
    set_capability("access", 0)
    add_shortcut("ytbridge")
    set_callbacks(Open, Close)
vlc_module_end()
