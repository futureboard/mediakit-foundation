#include "decoder.h"

#include <CoreMedia/CoreMedia.h>
#include <CoreVideo/CoreVideo.h>
#include <VideoToolbox/VideoToolbox.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "decoder_shared.h"
#include "platform_api_table.h"
#include "platform_context.h"
#include "src/codecs/h264/h264_bitstream.h"
#include "src/codecs/h264/h264_slice.h"
#include "src/codecs/h264/h264_sps_pps.h"
#include "src/common/mkff_log_util.h"
#include "video_frame.h"

#define DEFAULT_POOL_MIN 6
#define DEFAULT_POOL_MAX 16
#define READY_QUEUE_CAPACITY 32

/* Raw NAL bytes for SPS/PPS, cached so CMVideoFormatDescriptionCreate-
 * FromH264ParameterSets() can be called once the first IDR arrives —
 * it needs the *original* encoded parameter sets, not just our parsed
 * H264SPS/H264PPS field structs. */
typedef struct RawNal {
    uint8_t *data;
    size_t   size;
    int      valid;
} RawNal;

typedef struct MacosVideoDecoder {
    MKFF_HandleCommon common; /* must be first */

    DecoderShared *shared;
    int            initialized;
    MKFF_VideoProfile mkff_profile;

    H264SPS  sps_table[H264_MAX_SPS_COUNT];
    H264PPS  pps_table[H264_MAX_PPS_COUNT];
    RawNal   sps_raw[H264_MAX_SPS_COUNT];
    RawNal   pps_raw[H264_MAX_PPS_COUNT];
    H264PocState poc_state;

    uint32_t coded_width, coded_height;
    uint32_t display_width, display_height;

    uint32_t requested_max_surfaces;

    pthread_mutex_t     ready_lock;
    MacosVideoFrame     *ready_queue[READY_QUEUE_CAPACITY];
    uint32_t              ready_count;
    int                    flushed;

    MKFF_LogCallback log_callback;
    void            *log_user_data;
    MKFF_LogLevel    log_min_level;

    char last_error[256];
} MacosVideoDecoder;

#define DLOG(dec, lvl, ...) MKFF_LOG((dec)->log_callback, (dec)->log_user_data, (dec)->log_min_level, (lvl), "mkff.platform.macos.h264", __VA_ARGS__)

static void set_last_error(MacosVideoDecoder *dec, const char *msg) {
    snprintf(dec->last_error, sizeof(dec->last_error), "%s", msg);
    DLOG(dec, MKFF_LOG_LEVEL_ERROR, "%s", msg);
}

static void cache_raw_nal(RawNal *slot, const uint8_t *data, size_t size) {
    free(slot->data);
    slot->data = (uint8_t *)malloc(size);
    if (!slot->data) {
        slot->valid = 0;
        return;
    }
    memcpy(slot->data, data, size);
    slot->size = size;
    slot->valid = 1;
}

/* sourceFrameRefCon payload: carries the caller-supplied timestamps
 * (distinct from the POC-derived CMTime we hand VideoToolbox purely to
 * drive its internal temporal reordering) through to the output
 * callback, plus enough context to build the MKFF_VideoFrame and route
 * it back to the right decoder. */
typedef struct PendingFrame {
    MacosVideoDecoder *dec;
    int64_t pts;
    int64_t dts;
    int     is_key_frame;
} PendingFrame;

static void decode_output_callback(void *decompressionOutputRefCon,
                                    void *sourceFrameRefCon,
                                    OSStatus status,
                                    VTDecodeInfoFlags infoFlags,
                                    CVImageBufferRef imageBuffer,
                                    CMTime presentationTimeStamp,
                                    CMTime presentationDuration) {
    (void)decompressionOutputRefCon;
    (void)infoFlags;
    (void)presentationTimeStamp;
    (void)presentationDuration;

    PendingFrame *pending = (PendingFrame *)sourceFrameRefCon;
    MacosVideoDecoder *dec = pending->dec;

    if (status != noErr || !imageBuffer) {
        DLOG(dec, MKFF_LOG_LEVEL_WARN, "VTDecompressionSession output callback reported status=%d (frame dropped)", (int)status);
        decoder_shared_pool_release(dec->shared);
        free(pending);
        return;
    }

    CVPixelBufferRef pixel_buffer = (CVPixelBufferRef)imageBuffer;
    CVPixelBufferRetain(pixel_buffer);

    MacosVideoFrame *frame = macos_video_frame_create(dec->shared, pixel_buffer, dec->display_width, dec->display_height, pending->pts, pending->dts, pending->is_key_frame);
    free(pending);

    if (!frame) {
        CVPixelBufferRelease(pixel_buffer);
        decoder_shared_pool_release(dec->shared);
        return;
    }

    pthread_mutex_lock(&dec->ready_lock);
    if (dec->ready_count < READY_QUEUE_CAPACITY) {
        dec->ready_queue[dec->ready_count++] = frame;
    } else {
        DLOG(dec, MKFF_LOG_LEVEL_WARN, "%s", "ready queue full: dropping a decoded frame (caller is not calling receive() often enough)");
        macos_video_frame_release((MKFF_VideoFrame *)frame);
    }
    pthread_mutex_unlock(&dec->ready_lock);
}

static MKFF_Result ensure_vt_initialized(MacosVideoDecoder *dec, const H264SPS *sps, const H264PPS *pps) {
    uint32_t coded_w = sps->pic_width_in_mbs * 16;
    uint32_t coded_h = sps->pic_height_in_map_units * 16;

    if (dec->initialized) {
        if (coded_w != dec->coded_width || coded_h != dec->coded_height) {
            set_last_error(dec, "mid-stream resolution change is not supported in this milestone");
            return MKFF_RESULT_ERROR_NOT_SUPPORTED;
        }
        return MKFF_RESULT_OK;
    }

    const RawNal *sps_raw = &dec->sps_raw[sps->seq_parameter_set_id];
    const RawNal *pps_raw = &dec->pps_raw[pps->pic_parameter_set_id];
    if (!sps_raw->valid || !pps_raw->valid) {
        set_last_error(dec, "internal error: SPS/PPS raw NAL bytes not cached before first IDR");
        return MKFF_RESULT_ERROR_INTERNAL;
    }

    const uint8_t *param_ptrs[2] = {sps_raw->data, pps_raw->data};
    const size_t param_sizes[2] = {sps_raw->size, pps_raw->size};

    CMVideoFormatDescriptionRef format_description = NULL;
    OSStatus status = CMVideoFormatDescriptionCreateFromH264ParameterSets(kCFAllocatorDefault, 2, param_ptrs, param_sizes, 4, &format_description);
    if (status != noErr) {
        set_last_error(dec, "CMVideoFormatDescriptionCreateFromH264ParameterSets failed");
        return MKFF_RESULT_ERROR_DEVICE;
    }

    CFMutableDictionaryRef iosurface_props = CFDictionaryCreateMutable(kCFAllocatorDefault, 0, &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);

    SInt32 pixel_format = kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange;
    CFNumberRef pixel_format_num = CFNumberCreate(kCFAllocatorDefault, kCFNumberSInt32Type, &pixel_format);

    CFMutableDictionaryRef dest_attrs = CFDictionaryCreateMutable(kCFAllocatorDefault, 0, &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
    CFDictionarySetValue(dest_attrs, kCVPixelBufferPixelFormatTypeKey, pixel_format_num);
    CFDictionarySetValue(dest_attrs, kCVPixelBufferIOSurfacePropertiesKey, iosurface_props);
    CFRelease(pixel_format_num);
    CFRelease(iosurface_props);

    VTDecompressionOutputCallbackRecord callback;
    callback.decompressionOutputCallback = decode_output_callback;
    callback.decompressionOutputRefCon = dec;

    VTDecompressionSessionRef session = NULL;
    status = VTDecompressionSessionCreate(kCFAllocatorDefault, format_description, NULL, dest_attrs, &callback, &session);
    CFRelease(dest_attrs);
    if (status != noErr) {
        CFRelease(format_description);
        set_last_error(dec, "VTDecompressionSessionCreate failed");
        return MKFF_RESULT_ERROR_DEVICE;
    }

    uint32_t pool_capacity = dec->requested_max_surfaces;
    if (pool_capacity == 0) {
        uint32_t base = sps->max_num_ref_frames + 4;
        pool_capacity = base < DEFAULT_POOL_MIN ? DEFAULT_POOL_MIN : (base > DEFAULT_POOL_MAX ? DEFAULT_POOL_MAX : base);
    }

    DecoderShared *shared = decoder_shared_create(pool_capacity, dec->log_callback, dec->log_user_data, dec->log_min_level);
    if (!shared) {
        VTDecompressionSessionInvalidate(session);
        CFRelease(session);
        CFRelease(format_description);
        set_last_error(dec, "out of memory allocating decoder state");
        return MKFF_RESULT_ERROR_OUT_OF_MEMORY;
    }
    shared->session = session;
    shared->format_description = format_description;
    shared->initialized = 1;
    shared->coded_width = coded_w;
    shared->coded_height = coded_h;

    dec->shared = shared;
    dec->initialized = 1;
    dec->mkff_profile = MKFF_VIDEO_PROFILE_H264_HIGH; /* VideoToolbox negotiates Baseline/Main/High uniformly */
    dec->coded_width = coded_w;
    dec->coded_height = coded_h;

    DLOG(dec, MKFF_LOG_LEVEL_INFO, "initialized VideoToolbox decode: %ux%u pool_capacity=%u", coded_w, coded_h, pool_capacity);
    return MKFF_RESULT_OK;
}

MKFF_Result macos_video_decoder_create(MKFF_PlatformContext *pctx, const MKFF_VideoDecoderDesc *desc, void **out_decoder) {
    if (desc->codec != MKFF_VIDEO_CODEC_H264) {
        return MKFF_RESULT_ERROR_NOT_SUPPORTED;
    }

    MacosVideoDecoder *dec = (MacosVideoDecoder *)calloc(1, sizeof(MacosVideoDecoder));
    if (!dec) {
        return MKFF_RESULT_ERROR_OUT_OF_MEMORY;
    }

    dec->common.api = mkff_macos_platform_api();
    dec->log_callback = pctx->log_callback;
    dec->log_user_data = pctx->log_user_data;
    dec->log_min_level = pctx->log_min_level;
    dec->requested_max_surfaces = desc->max_surfaces;
    pthread_mutex_init(&dec->ready_lock, NULL);

    *out_decoder = dec;
    return MKFF_RESULT_OK;
}

void macos_video_decoder_destroy(void *decoder_v) {
    MacosVideoDecoder *dec = (MacosVideoDecoder *)decoder_v;
    if (!dec) return;

    if (dec->shared && dec->shared->session) {
        VTDecompressionSessionFinishDelayedFrames(dec->shared->session);
    }

    for (uint32_t i = 0; i < dec->ready_count; i++) {
        macos_video_frame_release((MKFF_VideoFrame *)dec->ready_queue[i]);
    }
    pthread_mutex_destroy(&dec->ready_lock);

    for (uint32_t i = 0; i < H264_MAX_SPS_COUNT; i++) free(dec->sps_raw[i].data);
    for (uint32_t i = 0; i < H264_MAX_PPS_COUNT; i++) free(dec->pps_raw[i].data);

    if (dec->shared) {
        decoder_shared_unref(dec->shared);
    }
    free(dec);
}

/* Converts Annex-B slice NALs (start-code delimited) accumulated for
 * one access unit into an AVCC CMBlockBuffer (4-byte big-endian length
 * prefix per NAL, no start codes) — the sample format
 * VTDecompressionSessionDecodeFrame expects. */
static CMBlockBufferRef build_avcc_block_buffer(const H264NalUnit *nals, size_t nal_count, size_t total_size) {
    uint8_t *avcc = (uint8_t *)malloc(total_size);
    if (!avcc) return NULL;

    size_t offset = 0;
    for (size_t i = 0; i < nal_count; i++) {
        uint32_t len = (uint32_t)nals[i].size;
        avcc[offset + 0] = (uint8_t)(len >> 24);
        avcc[offset + 1] = (uint8_t)(len >> 16);
        avcc[offset + 2] = (uint8_t)(len >> 8);
        avcc[offset + 3] = (uint8_t)(len >> 0);
        memcpy(avcc + offset + 4, nals[i].data, nals[i].size);
        offset += 4 + nals[i].size;
    }

    CMBlockBufferRef block_buffer = NULL;
    OSStatus status = CMBlockBufferCreateWithMemoryBlock(kCFAllocatorDefault, avcc, total_size, kCFAllocatorDefault, NULL, 0, total_size, 0, &block_buffer);
    if (status != noErr) {
        free(avcc);
        return NULL;
    }
    /* `avcc` ownership transferred to the block buffer (freed via the
     * kCFAllocatorDefault deallocator when the block buffer is
     * released), since we passed it as the memoryBlock with
     * blockAllocator == kCFAllocatorDefault. */
    return block_buffer;
}

MKFF_Result macos_video_decoder_submit(void *decoder_v, const uint8_t *annex_b_data, size_t annex_b_size, int64_t pts, int64_t dts) {
    MacosVideoDecoder *dec = (MacosVideoDecoder *)decoder_v;

    if (annex_b_size == 0) {
        return MKFF_RESULT_OK;
    }

    H264NalUnit nals[H264_MAX_NAL_UNITS_PER_AU];
    size_t nal_count = h264_split_annex_b(annex_b_data, annex_b_size, nals, H264_MAX_NAL_UNITS_PER_AU);
    if (nal_count == 0) {
        set_last_error(dec, "no NAL units found (missing Annex-B start code)");
        return MKFF_RESULT_ERROR_BITSTREAM;
    }

    uint8_t *rbsp_scratch = (uint8_t *)malloc(annex_b_size);
    if (!rbsp_scratch) {
        return MKFF_RESULT_ERROR_OUT_OF_MEMORY;
    }

    H264NalUnit slice_nals[H264_MAX_NAL_UNITS_PER_AU];
    size_t slice_nal_count = 0;
    size_t slice_bytes_total = 0;

    int have_picture = 0;
    H264SliceHeader first_sh;
    const H264SPS *pic_sps = NULL;
    const H264PPS *pic_pps = NULL;
    MKFF_Result result = MKFF_RESULT_OK;
    int is_idr_au = 0;

    for (size_t i = 0; i < nal_count && result == MKFF_RESULT_OK; i++) {
        const H264NalUnit *nal = &nals[i];

        if (nal->nal_unit_type == 7) { /* SPS */
            H264SPS tmp;
            int rc = h264_parse_sps(nal->data, nal->size, rbsp_scratch, &tmp);
            if (rc == 0 && tmp.seq_parameter_set_id < H264_MAX_SPS_COUNT) {
                dec->sps_table[tmp.seq_parameter_set_id] = tmp;
                cache_raw_nal(&dec->sps_raw[tmp.seq_parameter_set_id], nal->data, nal->size);
            } else if (tmp.seq_parameter_set_id < H264_MAX_SPS_COUNT) {
                DLOG(dec, MKFF_LOG_LEVEL_WARN, "rejecting SPS id=%u (rc=%d): outside this milestone's supported subset", tmp.seq_parameter_set_id, rc);
                dec->sps_table[tmp.seq_parameter_set_id].valid = 0;
            }
            continue;
        }

        if (nal->nal_unit_type == 8) { /* PPS */
            H264PPS tmp;
            int rc = h264_parse_pps(nal->data, nal->size, rbsp_scratch, dec->sps_table, &tmp);
            if (rc == 0 && tmp.pic_parameter_set_id < H264_MAX_PPS_COUNT) {
                dec->pps_table[tmp.pic_parameter_set_id] = tmp;
                cache_raw_nal(&dec->pps_raw[tmp.pic_parameter_set_id], nal->data, nal->size);
            } else if (tmp.pic_parameter_set_id < H264_MAX_PPS_COUNT) {
                DLOG(dec, MKFF_LOG_LEVEL_WARN, "rejecting PPS id=%u (rc=%d): outside this milestone's supported subset", tmp.pic_parameter_set_id, rc);
                dec->pps_table[tmp.pic_parameter_set_id].valid = 0;
            }
            continue;
        }

        if (nal->nal_unit_type != 1 && nal->nal_unit_type != 5) {
            continue;
        }

        H264SliceHeader sh;
        int rc = h264_parse_slice_header(nal, rbsp_scratch, dec->sps_table, dec->pps_table, &sh);
        if (rc != 0) {
            DLOG(dec, MKFF_LOG_LEVEL_WARN, "dropping slice NAL (rc=%d)", rc);
            continue;
        }

        const H264PPS *pps = &dec->pps_table[sh.pic_parameter_set_id];
        const H264SPS *sps = &dec->sps_table[pps->seq_parameter_set_id];

        if (!have_picture) {
            if (!dec->initialized && !sh.is_idr) {
                DLOG(dec, MKFF_LOG_LEVEL_DEBUG, "%s", "dropping access unit before the first IDR");
                break;
            }

            MKFF_Result init_result = ensure_vt_initialized(dec, sps, pps);
            if (init_result != MKFF_RESULT_OK) {
                result = init_result;
                break;
            }

            uint32_t crop_unit_x = 2;
            uint32_t crop_unit_y = 2;
            dec->display_width = dec->coded_width;
            dec->display_height = dec->coded_height;
            if (sps->frame_cropping_flag) {
                dec->display_width -= (sps->crop_left + sps->crop_right) * crop_unit_x;
                dec->display_height -= (sps->crop_top + sps->crop_bottom) * crop_unit_y;
            }
            dec->shared->display_width = dec->display_width;
            dec->shared->display_height = dec->display_height;

            first_sh = sh;
            pic_sps = sps;
            pic_pps = pps;
            is_idr_au = sh.is_idr;
            have_picture = 1;
        }

        if (slice_nal_count < H264_MAX_NAL_UNITS_PER_AU) {
            slice_nals[slice_nal_count++] = *nal;
            slice_bytes_total += 4 + nal->size; /* AVCC 4-byte length prefix */
        }
    }

    if (have_picture && result == MKFF_RESULT_OK) {
        (void)pic_pps;

        if (decoder_shared_pool_checkout(dec->shared) != 0) {
            set_last_error(dec, "self-imposed surface cap reached: caller must release frames before submitting more");
            free(rbsp_scratch);
            return MKFF_RESULT_ERROR_POOL_EXHAUSTED;
        }

        int32_t poc = h264_compute_poc(pic_sps, &first_sh, &dec->poc_state);

        CMBlockBufferRef block_buffer = build_avcc_block_buffer(slice_nals, slice_nal_count, slice_bytes_total);
        if (!block_buffer) {
            decoder_shared_pool_release(dec->shared);
            free(rbsp_scratch);
            return MKFF_RESULT_ERROR_OUT_OF_MEMORY;
        }

        CMSampleTimingInfo timing;
        timing.duration = kCMTimeInvalid;
        timing.presentationTimeStamp = CMTimeMake(poc, 1); /* drives VideoToolbox's own temporal reordering only */
        timing.decodeTimeStamp = kCMTimeInvalid;

        size_t sample_size = slice_bytes_total;
        CMSampleBufferRef sample_buffer = NULL;
        OSStatus status = CMSampleBufferCreateReady(kCFAllocatorDefault, block_buffer, dec->shared->format_description, 1, 1, &timing, 1, &sample_size, &sample_buffer);
        CFRelease(block_buffer);

        if (status != noErr || !sample_buffer) {
            set_last_error(dec, "CMSampleBufferCreateReady failed");
            decoder_shared_pool_release(dec->shared);
            free(rbsp_scratch);
            return MKFF_RESULT_ERROR_DEVICE;
        }

        PendingFrame *pending = (PendingFrame *)malloc(sizeof(PendingFrame));
        pending->dec = dec;
        pending->pts = pts;
        pending->dts = dts;
        pending->is_key_frame = is_idr_au;

        VTDecodeFrameFlags decode_flags = kVTDecodeFrame_EnableTemporalProcessing;
        VTDecodeInfoFlags info_flags = 0;
        status = VTDecompressionSessionDecodeFrame(dec->shared->session, sample_buffer, decode_flags, pending, &info_flags);
        CFRelease(sample_buffer);

        if (status != noErr) {
            set_last_error(dec, "VTDecompressionSessionDecodeFrame failed");
            decoder_shared_pool_release(dec->shared);
            free(pending);
            result = MKFF_RESULT_ERROR_DECODE;
        }
    }

    free(rbsp_scratch);
    return result;
}

MKFF_Result macos_video_decoder_receive(void *decoder_v, MKFF_VideoFrame **out_frame) {
    MacosVideoDecoder *dec = (MacosVideoDecoder *)decoder_v;

    pthread_mutex_lock(&dec->ready_lock);
    if (dec->ready_count > 0) {
        *out_frame = (MKFF_VideoFrame *)dec->ready_queue[0];
        for (uint32_t i = 1; i < dec->ready_count; i++) {
            dec->ready_queue[i - 1] = dec->ready_queue[i];
        }
        dec->ready_count--;
        pthread_mutex_unlock(&dec->ready_lock);
        return MKFF_RESULT_OK;
    }
    pthread_mutex_unlock(&dec->ready_lock);

    return dec->flushed ? MKFF_RESULT_END_OF_STREAM : MKFF_RESULT_NOT_READY;
}

MKFF_Result macos_video_decoder_flush(void *decoder_v) {
    MacosVideoDecoder *dec = (MacosVideoDecoder *)decoder_v;

    if (dec->shared && dec->shared->session) {
        VTDecompressionSessionFinishDelayedFrames(dec->shared->session);
    }
    dec->flushed = 1;
    return MKFF_RESULT_OK;
}

MKFF_Result macos_video_decoder_get_info(const void *decoder_v, MKFF_VideoDecoderInfo *out_info) {
    const MacosVideoDecoder *dec = (const MacosVideoDecoder *)decoder_v;

    uint32_t requested_size = out_info->struct_size;
    memset(out_info, 0, sizeof(*out_info));
    MKFF_INIT_STRUCT_HEADER(out_info);
    out_info->struct_size = requested_size ? requested_size : (uint32_t)sizeof(*out_info);

    out_info->width = dec->display_width;
    out_info->height = dec->display_height;
    out_info->profile = dec->mkff_profile;
    out_info->entrypoint = dec->initialized ? MKFF_VIDEO_ENTRYPOINT_VLD : MKFF_VIDEO_ENTRYPOINT_UNKNOWN;
    out_info->output_format = MKFF_PIXEL_FORMAT_NV12;

    if (dec->shared) {
        pthread_mutex_lock(&dec->shared->live_lock);
        out_info->surface_pool_size = dec->shared->live_count;
        out_info->surface_pool_capacity = dec->shared->pool_capacity;
        pthread_mutex_unlock(&dec->shared->live_lock);
    }

    return MKFF_RESULT_OK;
}
