#ifndef MKFF_TYPES_H
#define MKFF_TYPES_H

#include <stdint.h>

/* Opaque handles. Never dereferenced by API consumers. */
typedef struct MKFF_Context      MKFF_Context;
typedef struct MKFF_VideoDecoder MKFF_VideoDecoder;
typedef struct MKFF_VideoFrame   MKFF_VideoFrame;

typedef enum MKFF_PixelFormat {
    MKFF_PIXEL_FORMAT_UNKNOWN = 0,
    MKFF_PIXEL_FORMAT_NV12    = 1
} MKFF_PixelFormat;

typedef enum MKFF_VideoCodec {
    MKFF_VIDEO_CODEC_UNKNOWN = 0,
    MKFF_VIDEO_CODEC_H264    = 1,
    MKFF_VIDEO_CODEC_HEVC    = 2,
    MKFF_VIDEO_CODEC_VP9     = 3,
    MKFF_VIDEO_CODEC_AV1     = 4
} MKFF_VideoCodec;

typedef enum MKFF_VideoProfile {
    MKFF_VIDEO_PROFILE_UNKNOWN          = 0,

    MKFF_VIDEO_PROFILE_H264_BASELINE    = 100,
    MKFF_VIDEO_PROFILE_H264_MAIN        = 101,
    MKFF_VIDEO_PROFILE_H264_HIGH        = 102,
    MKFF_VIDEO_PROFILE_H264_CONSTRAINED_BASELINE = 103,

    MKFF_VIDEO_PROFILE_HEVC_MAIN        = 200,
    MKFF_VIDEO_PROFILE_HEVC_MAIN10      = 201,

    MKFF_VIDEO_PROFILE_VP9_PROFILE0     = 300,
    MKFF_VIDEO_PROFILE_VP9_PROFILE2     = 301,

    MKFF_VIDEO_PROFILE_AV1_MAIN         = 400
} MKFF_VideoProfile;

typedef enum MKFF_VideoEntrypoint {
    MKFF_VIDEO_ENTRYPOINT_UNKNOWN = 0,
    MKFF_VIDEO_ENTRYPOINT_VLD     = 1  /* Variable Length Decoding (hw slice/entropy decode) */
} MKFF_VideoEntrypoint;

#endif /* MKFF_TYPES_H */
