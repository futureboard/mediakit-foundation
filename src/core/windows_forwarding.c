#include "mkff/windows/d3d11.h"

#include <string.h>
#include <unknwn.h>
#include <windows.h>

#include "context_internal.h"
#include "src/common/mkff_common.h"

MKFF_Result mkff_windows_video_frame_export_d3d11_texture(const MKFF_VideoFrame *frame, MKFF_WindowsD3D11TextureDesc *out_desc) {
    if (!frame || !out_desc) {
        return MKFF_RESULT_ERROR_INVALID_ARGUMENT;
    }
    const MKFF_HandleCommon *common = (const MKFF_HandleCommon *)frame;
    if (!common->api->video_frame_export_d3d11_texture) {
        return MKFF_RESULT_ERROR_NOT_SUPPORTED;
    }
    return common->api->video_frame_export_d3d11_texture(frame, out_desc);
}

void mkff_windows_d3d11_texture_desc_close(MKFF_WindowsD3D11TextureDesc *desc) {
    if (!desc) return;
    if (desc->texture) {
        IUnknown_Release((IUnknown *)desc->texture);
    }
    if (desc->shared_handle) {
        CloseHandle(desc->shared_handle);
    }
    memset(desc, 0, sizeof(*desc));
}
