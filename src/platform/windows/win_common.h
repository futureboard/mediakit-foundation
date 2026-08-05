#ifndef MKFF_WINDOWS_WIN_COMMON_H
#define MKFF_WINDOWS_WIN_COMMON_H

/*
 * Single include point for the Windows/D3D11/DXVA headers so every
 * translation unit in this platform module sees the same macro setup:
 *
 *   WIN32_LEAN_AND_MEAN - skip rarely-needed Win32 API surface
 *   COBJMACROS          - enables the C-callable
 *                          Interface_Method(instance, args...) macros
 *                          for COM calls (otherwise every call site
 *                          would need the more verbose
 *                          instance->lpVtbl->Method(instance, args...)
 *                          form)
 *
 * DXVA_PicParams_H264 / DXVA_Qmatrix_H264 / DXVA_Slice_H264_Short /
 * DXVA_PicEntry_H264 come from <dxva.h> — the same structures DXVA2
 * and D3D11 video decode share verbatim (Microsoft documents this
 * explicitly; it's also exactly what FFmpeg's d3d11va_h264.c and
 * dxva2_h264.c both do). The H.264 decoder profile GUID
 * (DXVA2_ModeH264_VLD_NoFGT) similarly comes from <dxva2api.h> even
 * though this module never uses the DXVA2 IDirectXVideoDecoder API
 * itself — only its profile identifier constants.
 */

#define WIN32_LEAN_AND_MEAN
#define COBJMACROS

#include <d3d11.h>
#include <dxgi1_2.h>
#include <dxva.h>
#include <dxva2api.h>
#include <windows.h>

#endif /* MKFF_WINDOWS_WIN_COMMON_H */
