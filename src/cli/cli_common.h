#ifndef MKFF_CLI_COMMON_H
#define MKFF_CLI_COMMON_H

#include <stddef.h>
#include <stdint.h>

#include "mkff/linux/dmabuf.h"
#include "mkff/linux/drm.h"
#include "mkff/linux/va.h"
#include "mkff/mkff.h"

int cli_read_file(const char *path, uint8_t **out_data, size_t *out_size);

typedef struct CliAuRange {
    size_t offset;
    size_t size;
} CliAuRange;

/*
 * Splits an Annex-B buffer into access-unit-sized chunks suitable for
 * one mkff_video_decoder_submit() call each. Heuristic: a new access
 * unit starts at each VCL NAL (type 1 or 5) that follows another VCL
 * NAL already accumulated into the current chunk; leading non-VCL NALs
 * (SPS/PPS/SEI/AUD) attach to the following access unit. This is exact
 * for one-slice-per-picture streams (the common case, and what this
 * milestone's H.264 scope targets); multi-slice pictures would each be
 * (mis-)split into separate access units by this CLI-side heuristic —
 * a CLI simplification, not a library limitation.
 */
size_t cli_split_access_units(const uint8_t *data, size_t size, CliAuRange *out_ranges, size_t capacity);

double cli_now_seconds(void);

void cli_log_callback(void *user_data, MKFF_LogLevel level, const char *component, const char *message);

const char *cli_pixel_format_name(MKFF_PixelFormat format);
const char *cli_profile_name(MKFF_VideoProfile profile);
const char *cli_codec_name(MKFF_VideoCodec codec);

#endif /* MKFF_CLI_COMMON_H */
