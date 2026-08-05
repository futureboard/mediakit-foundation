set(MKFF_CORE_SOURCES
    context.c
    error.c
    linux_forwarding.c
    platform_loader.c
    video_decoder.c
    video_frame.c
    cpu_features.c
    simd/checksum.c
    simd/checksum_avx2.S
)

add_library(mkff SHARED ${MKFF_CORE_SOURCES})

target_include_directories(mkff
    PUBLIC
        $<BUILD_INTERFACE:${PROJECT_SOURCE_DIR}/include>
        $<INSTALL_INTERFACE:include>
    PRIVATE
        ${PROJECT_SOURCE_DIR}
)

target_compile_definitions(mkff PRIVATE MKFF_BUILDING_SHARED)
target_link_libraries(mkff PRIVATE ${CMAKE_DL_LIBS})

set_target_properties(mkff PROPERTIES
    VERSION ${PROJECT_VERSION}
    SOVERSION 0
    C_VISIBILITY_PRESET hidden
)

install(TARGETS mkff
    LIBRARY DESTINATION lib
    ARCHIVE DESTINATION lib
    RUNTIME DESTINATION bin
)
