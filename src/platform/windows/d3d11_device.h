#ifndef MKFF_WINDOWS_D3D11_DEVICE_H
#define MKFF_WINDOWS_D3D11_DEVICE_H

#include <stddef.h>

#include "mkff/error.h"
#include "win_common.h"

/* Creates a hardware D3D11 device with video-decode support
 * (D3D11_CREATE_DEVICE_VIDEO_SUPPORT). */
MKFF_Result windows_create_d3d11_device(ID3D11Device **out_device, ID3D11DeviceContext **out_device_context, char *err_buf, size_t err_buf_size);

/* Searches the device's supported decoder profiles for H.264 VLD
 * (no film grain). Returns 1 and fills *out_guid on success, 0 if
 * unsupported. */
int windows_select_h264_decoder_guid(ID3D11VideoDevice *video_device, GUID *out_guid);

/* Checks that DXGI_FORMAT_NV12 output is supported for `profile`. */
int windows_check_nv12_supported(ID3D11VideoDevice *video_device, const GUID *profile);

#endif /* MKFF_WINDOWS_D3D11_DEVICE_H */
