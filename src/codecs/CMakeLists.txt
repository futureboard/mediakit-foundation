# Portable H.264 bitstream/SPS/PPS/slice-header parsing (Annex-B NAL
# splitting, Exp-Golomb bit reader, POC derivation). No VA-API, D3D11,
# or VideoToolbox dependency: every platform backend links this same
# static library rather than re-implementing H.264 syntax parsing per
# platform.
add_library(mkff_codec_h264 STATIC
    h264/h264_bitstream.c
    h264/h264_sps_pps.c
    h264/h264_slice.c
)

target_include_directories(mkff_codec_h264 PUBLIC ${PROJECT_SOURCE_DIR})

set_target_properties(mkff_codec_h264 PROPERTIES
    C_VISIBILITY_PRESET hidden
    POSITION_INDEPENDENT_CODE ON
)
