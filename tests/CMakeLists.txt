include(CTest)

# --- test_context: exercises the public libmkff API ---
add_executable(test_context test_context.c)
target_include_directories(test_context PRIVATE ${PROJECT_SOURCE_DIR}/include)
target_link_libraries(test_context PRIVATE mkff)
add_test(NAME test_context COMMAND test_context)

# --- test_bitreader / test_h264_sps: white-box tests of the H.264
# parser internals, compiled directly against the platform module's
# sources so they need neither a VA-API display nor a DRM device. ---
add_executable(test_bitreader
    test_bitreader.c
    ${PROJECT_SOURCE_DIR}/src/platform/linux/h264/h264_bitstream.c
)
target_include_directories(test_bitreader PRIVATE ${PROJECT_SOURCE_DIR} ${PROJECT_SOURCE_DIR}/tests)
add_test(NAME test_bitreader COMMAND test_bitreader)

add_executable(test_h264_sps
    test_h264_sps.c
    ${PROJECT_SOURCE_DIR}/src/platform/linux/h264/h264_bitstream.c
    ${PROJECT_SOURCE_DIR}/src/platform/linux/h264/h264_sps_pps.c
)
target_include_directories(test_h264_sps PRIVATE ${PROJECT_SOURCE_DIR} ${PROJECT_SOURCE_DIR}/tests)
add_test(NAME test_h264_sps COMMAND test_h264_sps)

# --- test_checksum: SIMD dispatch (scalar vs AVX2) ---
add_executable(test_checksum
    test_checksum.c
    ${PROJECT_SOURCE_DIR}/src/core/simd/checksum.c
    ${PROJECT_SOURCE_DIR}/src/core/simd/checksum_avx2.S
    ${PROJECT_SOURCE_DIR}/src/core/cpu_features.c
)
target_include_directories(test_checksum PRIVATE ${PROJECT_SOURCE_DIR})
add_test(NAME test_checksum COMMAND test_checksum)

# --- test_surface_pool: bounded VA surface pool bookkeeping, GPU-free ---
add_executable(test_surface_pool
    test_surface_pool.c
    ${PROJECT_SOURCE_DIR}/src/platform/linux/decoder_shared.c
)
target_include_directories(test_surface_pool PRIVATE ${PROJECT_SOURCE_DIR} ${PROJECT_SOURCE_DIR}/include)
target_link_libraries(test_surface_pool PRIVATE PkgConfig::LIBVA Threads::Threads)
add_test(NAME test_surface_pool COMMAND test_surface_pool)

# --- CLI smoke tests: must not crash even with no GPU present; a clean
# "no device" exit(1) is an acceptable outcome, a crash is not. ---
find_program(BASH_PROGRAM bash)
if(BASH_PROGRAM)
    add_test(NAME cli_devices_smoke
             COMMAND ${BASH_PROGRAM} -c "$<TARGET_FILE:mkff_cli> devices; test $? -le 1")
    add_test(NAME cli_va_info_smoke
             COMMAND ${BASH_PROGRAM} -c "$<TARGET_FILE:mkff_cli> va-info; test $? -le 1")
    # Exercises Annex-B splitting + SPS/PPS/slice-header parsing +
    # decoder orchestration end to end via the real CLI binary. On a
    # host with no GPU (e.g. CI) this fails gracefully at device-open
    # with a clean error message; a crash is the only failure mode this
    # test rejects.
    add_test(NAME cli_decode_test_smoke
             COMMAND ${BASH_PROGRAM} -c "$<TARGET_FILE:mkff_cli> decode-test ${PROJECT_SOURCE_DIR}/testdata/tiny_baseline_64x64.h264; test $? -le 1")
endif()
