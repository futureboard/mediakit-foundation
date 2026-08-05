#include "mkff/video_frame.h"

#include "src/common/mkff_common.h"

static const MKFF_HandleCommon *as_common(const void *handle) {
    return (const MKFF_HandleCommon *)handle;
}

MKFF_VideoFrame *mkff_video_frame_retain(MKFF_VideoFrame *frame) {
    if (!frame) return NULL;
    return as_common(frame)->api->video_frame_retain(frame);
}

void mkff_video_frame_release(MKFF_VideoFrame *frame) {
    if (!frame) return;
    as_common(frame)->api->video_frame_release(frame);
}

MKFF_Result mkff_video_frame_get_info(const MKFF_VideoFrame *frame, MKFF_VideoFrameInfo *out_info) {
    if (!frame || !out_info) {
        return MKFF_RESULT_ERROR_INVALID_ARGUMENT;
    }
    return as_common(frame)->api->video_frame_get_info(frame, out_info);
}
