#include "video_frame.h"

#include <IOSurface/IOSurface.h>
#include <stdlib.h>
#include <string.h>

#include "decoder_shared.h"
#include "platform_api_table.h"

MacosVideoFrame *macos_video_frame_create(DecoderShared *shared,
                                           CVPixelBufferRef pixel_buffer,
                                           uint32_t width,
                                           uint32_t height,
                                           int64_t pts,
                                           int64_t dts,
                                           int is_key_frame) {
    MacosVideoFrame *frame = (MacosVideoFrame *)calloc(1, sizeof(MacosVideoFrame));
    if (!frame) return NULL;

    frame->common.api = mkff_macos_platform_api();
    frame->shared = decoder_shared_ref(shared);
    frame->pixel_buffer = pixel_buffer; /* ownership transferred in by the caller */
    atomic_init(&frame->refcount, 1);

    MKFF_INIT_STRUCT_HEADER(&frame->info);
    frame->info.width = width;
    frame->info.height = height;
    frame->info.format = MKFF_PIXEL_FORMAT_NV12;
    frame->info.pts = pts;
    frame->info.dts = dts;
    frame->info.is_key_frame = is_key_frame ? 1u : 0u;

    return frame;
}

MKFF_VideoFrame *macos_video_frame_retain(MKFF_VideoFrame *handle) {
    MacosVideoFrame *frame = (MacosVideoFrame *)handle;
    if (frame) {
        atomic_fetch_add_explicit(&frame->refcount, 1, memory_order_relaxed);
    }
    return handle;
}

void macos_video_frame_release(MKFF_VideoFrame *handle) {
    MacosVideoFrame *frame = (MacosVideoFrame *)handle;
    if (!frame) return;

    if (atomic_fetch_sub_explicit(&frame->refcount, 1, memory_order_acq_rel) == 1) {
        CVPixelBufferRelease(frame->pixel_buffer);
        decoder_shared_pool_release(frame->shared);
        decoder_shared_unref(frame->shared);
        free(frame);
    }
}

MKFF_Result macos_video_frame_get_info(const MKFF_VideoFrame *handle, MKFF_VideoFrameInfo *out_info) {
    const MacosVideoFrame *frame = (const MacosVideoFrame *)handle;
    uint32_t requested_size = out_info->struct_size;
    MKFF_INIT_STRUCT_HEADER(out_info);
    *out_info = frame->info;
    out_info->struct_size = requested_size ? requested_size : (uint32_t)sizeof(*out_info);
    return MKFF_RESULT_OK;
}

MKFF_Result macos_video_frame_export_iosurface(const MKFF_VideoFrame *handle, MKFF_MacosIOSurfaceDesc *out_desc) {
    const MacosVideoFrame *frame = (const MacosVideoFrame *)handle;

    IOSurfaceRef surface = CVPixelBufferGetIOSurface(frame->pixel_buffer);
    if (!surface) {
        return MKFF_RESULT_ERROR_NOT_SUPPORTED; /* pixel buffer isn't IOSurface-backed: outputPixelBufferAttributes misconfigured */
    }
    CFRetain(surface);

    uint32_t requested_size = out_desc->struct_size;
    memset(out_desc, 0, sizeof(*out_desc));
    MKFF_INIT_STRUCT_HEADER(out_desc);
    out_desc->struct_size = requested_size ? requested_size : (uint32_t)sizeof(*out_desc);

    out_desc->width = frame->info.width;
    out_desc->height = frame->info.height;
    out_desc->pixel_format = (uint32_t)CVPixelBufferGetPixelFormatType(frame->pixel_buffer);
    out_desc->io_surface_id = IOSurfaceGetID(surface);
    out_desc->surface = (void *)surface;

    return MKFF_RESULT_OK;
}
