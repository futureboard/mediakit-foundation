#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "cli_common.h"

int cmd_decode_test(int argc, char **argv) {
    if (argc < 2) {
        fprintf(stderr, "usage: decode-test <input.h264> [--frames N]\n");
        return 2;
    }
    const char *input_path = argv[1];
    long max_frames = -1; /* unlimited */

    for (int i = 2; i < argc; i++) {
        if (strcmp(argv[i], "--frames") == 0 && i + 1 < argc) {
            max_frames = strtol(argv[++i], NULL, 10);
        }
    }

    uint8_t *file_data = NULL;
    size_t file_size = 0;
    if (cli_read_file(input_path, &file_data, &file_size) != 0) {
        fprintf(stderr, "failed to read %s\n", input_path);
        return 1;
    }

    CliAuRange *aus = (CliAuRange *)malloc(sizeof(CliAuRange) * 65536);
    size_t au_count = cli_split_access_units(file_data, file_size, aus, 65536);
    if (au_count == 0) {
        fprintf(stderr, "no H.264 access units found in %s\n", input_path);
        free(aus);
        free(file_data);
        return 1;
    }

    MKFF_ContextDesc ctx_desc;
    MKFF_INIT_STRUCT_HEADER(&ctx_desc);
    ctx_desc.log_callback = cli_log_callback;
    ctx_desc.log_min_level = MKFF_LOG_LEVEL_WARN;

    MKFF_Context *ctx = NULL;
    MKFF_Result result = mkff_context_create(&ctx_desc, &ctx);
    if (result != MKFF_RESULT_OK) {
        fprintf(stderr, "mkff_context_create failed: %s\n", mkff_result_to_string(result));
        free(aus);
        free(file_data);
        return 1;
    }

    MKFF_DrmDeviceInfo device;
    uint32_t device_count = 0;
    mkff_linux_enumerate_drm_devices(ctx, &device, 1, &device_count);

    MKFF_VaInfo va_info;
    MKFF_INIT_STRUCT_HEADER(&va_info);
    mkff_linux_query_va_info(ctx, NULL, &va_info);

    MKFF_VideoDecoderDesc dec_desc;
    MKFF_INIT_STRUCT_HEADER(&dec_desc);
    dec_desc.codec = MKFF_VIDEO_CODEC_H264;

    MKFF_VideoDecoder *decoder = NULL;
    result = mkff_video_decoder_create(ctx, &dec_desc, &decoder);
    if (result != MKFF_RESULT_OK) {
        fprintf(stderr, "mkff_video_decoder_create failed: %s (%s)\n", mkff_result_to_string(result), mkff_context_get_last_error(ctx));
        mkff_context_destroy(ctx);
        free(aus);
        free(file_data);
        return 1;
    }

    uint64_t decoded_frames = 0;
    double start_time = -1.0;
    int stop = 0;

    for (size_t i = 0; i < au_count && !stop; i++) {
        result = mkff_video_decoder_submit(decoder, file_data + aus[i].offset, aus[i].size, (int64_t)i, (int64_t)i);
        if (result != MKFF_RESULT_OK) {
            fprintf(stderr, "submit failed on AU %zu: %s (%s)\n", i, mkff_result_to_string(result), mkff_context_get_last_error(ctx));
            break;
        }

        for (;;) {
            MKFF_VideoFrame *frame = NULL;
            result = mkff_video_decoder_receive(decoder, &frame);
            if (result == MKFF_RESULT_NOT_READY) break;
            if (result != MKFF_RESULT_OK) break;

            if (start_time < 0) start_time = cli_now_seconds();
            decoded_frames++;
            mkff_video_frame_release(frame);

            if (max_frames >= 0 && (long)decoded_frames >= max_frames) {
                stop = 1;
                break;
            }
        }
    }

    if (!stop) {
        mkff_video_decoder_flush(decoder);
        for (;;) {
            MKFF_VideoFrame *frame = NULL;
            result = mkff_video_decoder_receive(decoder, &frame);
            if (result != MKFF_RESULT_OK) break;
            if (start_time < 0) start_time = cli_now_seconds();
            decoded_frames++;
            mkff_video_frame_release(frame);
            if (max_frames >= 0 && (long)decoded_frames >= max_frames) break;
        }
    }

    double end_time = cli_now_seconds();

    MKFF_VideoDecoderInfo info;
    MKFF_INIT_STRUCT_HEADER(&info);
    mkff_video_decoder_get_info(decoder, &info);

    double elapsed = (start_time >= 0) ? (end_time - start_time) : 0.0;
    double fps = (elapsed > 0.0) ? (double)decoded_frames / elapsed : 0.0;

    printf("DRM device:        %s\n", device_count > 0 ? device.path : "(none found)");
    printf("VA vendor:         %s\n", va_info.vendor_string);
    printf("selected profile:  %s\n", cli_profile_name(info.profile));
    printf("entrypoint:        %s\n", info.entrypoint == MKFF_VIDEO_ENTRYPOINT_VLD ? "VLD" : "unknown");
    printf("resolution:        %ux%u\n", info.width, info.height);
    printf("decoded frames:    %llu\n", (unsigned long long)decoded_frames);
    printf("surface-pool size: %u (capacity %u)\n", info.surface_pool_size, info.surface_pool_capacity);
    printf("decode FPS:        %.2f\n", fps);
    printf("output format:     %s\n", cli_pixel_format_name(info.output_format));
    printf("CPU readback:      false\n");

    mkff_video_decoder_destroy(decoder);
    mkff_context_destroy(ctx);
    free(aus);
    free(file_data);
    return decoded_frames > 0 ? 0 : 1;
}
