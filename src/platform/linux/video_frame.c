#include "video_frame.h"

#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <va/va_drmcommon.h>

#include "decoder_shared.h"
#include "platform_api_table.h"

LinuxVideoFrame *linux_video_frame_create(DecoderShared *shared,
                                           uint32_t pool_index,
                                           VASurfaceID surface,
                                           uint32_t width,
                                           uint32_t height,
                                           int64_t pts,
                                           int64_t dts,
                                           int is_key_frame) {
    LinuxVideoFrame *frame = (LinuxVideoFrame *)calloc(1, sizeof(LinuxVideoFrame));
    if (!frame) return NULL;

    frame->common.api = mkff_linux_platform_api();
    frame->shared = decoder_shared_ref(shared);
    frame->pool_index = pool_index;
    frame->surface = surface;
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

MKFF_VideoFrame *linux_video_frame_retain(MKFF_VideoFrame *handle) {
    LinuxVideoFrame *frame = (LinuxVideoFrame *)handle;
    if (frame) {
        atomic_fetch_add_explicit(&frame->refcount, 1, memory_order_relaxed);
    }
    return handle;
}

void linux_video_frame_release(MKFF_VideoFrame *handle) {
    LinuxVideoFrame *frame = (LinuxVideoFrame *)handle;
    if (!frame) return;

    if (atomic_fetch_sub_explicit(&frame->refcount, 1, memory_order_acq_rel) == 1) {
        /* Last reference: the VA surface can now return to the pool. */
        decoder_shared_pool_release(frame->shared, frame->pool_index);
        decoder_shared_unref(frame->shared);
        free(frame);
    }
}

MKFF_Result linux_video_frame_get_info(const MKFF_VideoFrame *handle, MKFF_VideoFrameInfo *out_info) {
    const LinuxVideoFrame *frame = (const LinuxVideoFrame *)handle;
    uint32_t requested_size = out_info->struct_size;
    MKFF_INIT_STRUCT_HEADER(out_info);
    *out_info = frame->info;
    out_info->struct_size = requested_size ? requested_size : (uint32_t)sizeof(*out_info);
    return MKFF_RESULT_OK;
}

MKFF_Result linux_video_frame_export_dmabuf(const MKFF_VideoFrame *handle, MKFF_LinuxDmaBufDesc *out_desc) {
    const LinuxVideoFrame *frame = (const LinuxVideoFrame *)handle;
    DecoderShared *shared = frame->shared;

    /* vaExportSurfaceHandle() is a synchronization point equivalent to
     * vaSyncSurface(), so no explicit sync is needed here even though
     * decode already completed synchronously before this frame was
     * handed to the caller. */
    VADRMPRIMESurfaceDescriptor va_desc;
    memset(&va_desc, 0, sizeof(va_desc));

    VAStatus status = vaExportSurfaceHandle(shared->va_dpy,
                                             frame->surface,
                                             VA_SURFACE_ATTRIB_MEM_TYPE_DRM_PRIME_2,
                                             VA_EXPORT_SURFACE_READ_ONLY | VA_EXPORT_SURFACE_COMPOSED_LAYERS,
                                             &va_desc);
    if (status != VA_STATUS_SUCCESS) {
        decoder_shared_set_error(shared, vaErrorStr(status));
        return MKFF_RESULT_ERROR_DEVICE;
    }

    uint32_t requested_size = out_desc->struct_size;
    memset(out_desc, 0, sizeof(*out_desc));
    MKFF_INIT_STRUCT_HEADER(out_desc);
    out_desc->struct_size = requested_size ? requested_size : (uint32_t)sizeof(*out_desc);

    out_desc->drm_fourcc = va_desc.fourcc;
    out_desc->width = va_desc.width;
    out_desc->height = va_desc.height;

    out_desc->num_objects = va_desc.num_objects;
    if (out_desc->num_objects > MKFF_LINUX_DMABUF_MAX_OBJECTS) {
        out_desc->num_objects = MKFF_LINUX_DMABUF_MAX_OBJECTS;
    }
    for (uint32_t i = 0; i < out_desc->num_objects; i++) {
        out_desc->objects[i].fd = va_desc.objects[i].fd; /* already a fresh, caller-owned dup per vaExportSurfaceHandle contract */
        out_desc->objects[i].size = va_desc.objects[i].size;
        out_desc->objects[i].modifier = va_desc.objects[i].drm_format_modifier;
    }

    uint32_t plane_count = 0;
    for (uint32_t l = 0; l < va_desc.num_layers && plane_count < MKFF_LINUX_DMABUF_MAX_PLANES; l++) {
        for (uint32_t p = 0; p < va_desc.layers[l].num_planes && plane_count < MKFF_LINUX_DMABUF_MAX_PLANES; p++) {
            out_desc->planes[plane_count].object_index = va_desc.layers[l].object_index[p];
            out_desc->planes[plane_count].offset = va_desc.layers[l].offset[p];
            out_desc->planes[plane_count].pitch = va_desc.layers[l].pitch[p];
            plane_count++;
        }
    }
    out_desc->num_planes = plane_count;

    return MKFF_RESULT_OK;
}
