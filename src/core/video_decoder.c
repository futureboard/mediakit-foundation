#include "mkff/video_decoder.h"

#include <string.h>

#include "context_internal.h"
#include "src/common/mkff_common.h"

static const MKFF_HandleCommon *as_common(const void *handle) {
    return (const MKFF_HandleCommon *)handle;
}

MKFF_Result mkff_video_decoder_create(MKFF_Context *context,
                                       const MKFF_VideoDecoderDesc *desc,
                                       MKFF_VideoDecoder **out_decoder) {
    if (!out_decoder) {
        return MKFF_RESULT_ERROR_INVALID_ARGUMENT;
    }
    *out_decoder = NULL;

    if (!context || !desc || desc->struct_size < sizeof(uint32_t) * 2) {
        return MKFF_RESULT_ERROR_INVALID_ARGUMENT;
    }

    const MKFF_PlatformAPI *api = mkff_context_internal_get_platform_api(context);
    if (!api) {
        return MKFF_RESULT_ERROR_PLATFORM_LOAD;
    }

    void *decoder = NULL;
    MKFF_Result result = api->video_decoder_create(mkff_context_internal_get_platform_context(context), desc, &decoder);
    if (result != MKFF_RESULT_OK) {
        return result;
    }

    *out_decoder = (MKFF_VideoDecoder *)decoder;
    return MKFF_RESULT_OK;
}

void mkff_video_decoder_destroy(MKFF_VideoDecoder *decoder) {
    if (!decoder) return;
    as_common(decoder)->api->video_decoder_destroy(decoder);
}

MKFF_Result mkff_video_decoder_submit(MKFF_VideoDecoder *decoder,
                                       const uint8_t *annex_b_data,
                                       size_t annex_b_size,
                                       int64_t pts,
                                       int64_t dts) {
    if (!decoder || (!annex_b_data && annex_b_size > 0)) {
        return MKFF_RESULT_ERROR_INVALID_ARGUMENT;
    }
    return as_common(decoder)->api->video_decoder_submit(decoder, annex_b_data, annex_b_size, pts, dts);
}

MKFF_Result mkff_video_decoder_receive(MKFF_VideoDecoder *decoder, MKFF_VideoFrame **out_frame) {
    if (!out_frame) {
        return MKFF_RESULT_ERROR_INVALID_ARGUMENT;
    }
    *out_frame = NULL;
    if (!decoder) {
        return MKFF_RESULT_ERROR_INVALID_ARGUMENT;
    }
    return as_common(decoder)->api->video_decoder_receive(decoder, out_frame);
}

MKFF_Result mkff_video_decoder_flush(MKFF_VideoDecoder *decoder) {
    if (!decoder) {
        return MKFF_RESULT_ERROR_INVALID_ARGUMENT;
    }
    return as_common(decoder)->api->video_decoder_flush(decoder);
}

MKFF_Result mkff_video_decoder_get_info(const MKFF_VideoDecoder *decoder, MKFF_VideoDecoderInfo *out_info) {
    if (!decoder || !out_info) {
        return MKFF_RESULT_ERROR_INVALID_ARGUMENT;
    }
    return as_common(decoder)->api->video_decoder_get_info(decoder, out_info);
}
