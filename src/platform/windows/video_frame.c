#include "video_frame.h"

#include <stdlib.h>
#include <string.h>
#include <unknwn.h>

#include "decoder_shared.h"
#include "platform_api_table.h"

WindowsVideoFrame *windows_video_frame_create(DecoderShared *shared,
                                               uint32_t array_slice,
                                               uint32_t width,
                                               uint32_t height,
                                               int64_t pts,
                                               int64_t dts,
                                               int is_key_frame) {
    WindowsVideoFrame *frame = (WindowsVideoFrame *)calloc(1, sizeof(WindowsVideoFrame));
    if (!frame) return NULL;

    frame->common.api = mkff_windows_platform_api();
    frame->shared = decoder_shared_ref(shared);
    frame->array_slice = array_slice;
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

MKFF_VideoFrame *windows_video_frame_retain(MKFF_VideoFrame *handle) {
    WindowsVideoFrame *frame = (WindowsVideoFrame *)handle;
    if (frame) {
        atomic_fetch_add_explicit(&frame->refcount, 1, memory_order_relaxed);
    }
    return handle;
}

void windows_video_frame_release(MKFF_VideoFrame *handle) {
    WindowsVideoFrame *frame = (WindowsVideoFrame *)handle;
    if (!frame) return;

    if (atomic_fetch_sub_explicit(&frame->refcount, 1, memory_order_acq_rel) == 1) {
        decoder_shared_pool_release(frame->shared, frame->array_slice);
        decoder_shared_unref(frame->shared);
        free(frame);
    }
}

MKFF_Result windows_video_frame_get_info(const MKFF_VideoFrame *handle, MKFF_VideoFrameInfo *out_info) {
    const WindowsVideoFrame *frame = (const WindowsVideoFrame *)handle;
    uint32_t requested_size = out_info->struct_size;
    MKFF_INIT_STRUCT_HEADER(out_info);
    *out_info = frame->info;
    out_info->struct_size = requested_size ? requested_size : (uint32_t)sizeof(*out_info);
    return MKFF_RESULT_OK;
}

MKFF_Result windows_video_frame_export_d3d11_texture(const MKFF_VideoFrame *handle, MKFF_WindowsD3D11TextureDesc *out_desc) {
    const WindowsVideoFrame *frame = (const WindowsVideoFrame *)handle;
    DecoderShared *shared = frame->shared;

    IDXGIResource1 *dxgi_resource = NULL;
    HRESULT hr = IUnknown_QueryInterface((IUnknown *)shared->texture_array, &IID_IDXGIResource1, (void **)&dxgi_resource);
    if (FAILED(hr)) {
        decoder_shared_set_error(shared, "QueryInterface(IDXGIResource1) failed on the decoder's texture array");
        return MKFF_RESULT_ERROR_DEVICE;
    }

    HANDLE shared_handle = NULL;
    hr = IDXGIResource1_CreateSharedHandle(dxgi_resource, NULL, DXGI_SHARED_RESOURCE_READ | DXGI_SHARED_RESOURCE_WRITE, NULL, &shared_handle);
    IUnknown_Release((IUnknown *)dxgi_resource);
    if (FAILED(hr)) {
        decoder_shared_set_error(shared, "IDXGIResource1::CreateSharedHandle failed");
        return MKFF_RESULT_ERROR_DEVICE;
    }

    IUnknown_AddRef((IUnknown *)shared->texture_array);

    uint32_t requested_size = out_desc->struct_size;
    memset(out_desc, 0, sizeof(*out_desc));
    MKFF_INIT_STRUCT_HEADER(out_desc);
    out_desc->struct_size = requested_size ? requested_size : (uint32_t)sizeof(*out_desc);

    out_desc->width = frame->info.width;
    out_desc->height = frame->info.height;
    out_desc->dxgi_format = DXGI_FORMAT_NV12;
    out_desc->array_slice = frame->array_slice;
    out_desc->texture = shared->texture_array;
    out_desc->shared_handle = shared_handle;

    return MKFF_RESULT_OK;
}
