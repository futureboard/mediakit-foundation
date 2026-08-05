set(MKFF_CORE_SOURCES
    context.c
    error.c
    platform_loader.c
    video_decoder.c
    video_frame.c
    cpu_features.c
    simd/checksum.c
)

if(CMAKE_SYSTEM_NAME STREQUAL "Linux")
    list(APPEND MKFF_CORE_SOURCES linux_forwarding.c)
elseif(CMAKE_SYSTEM_NAME STREQUAL "Darwin")
    list(APPEND MKFF_CORE_SOURCES macos_forwarding.c)
elseif(CMAKE_SYSTEM_NAME STREQUAL "Windows")
    list(APPEND MKFF_CORE_SOURCES windows_forwarding.c)
endif()

if(WIN32)
    # No GNU-syntax assembler available under MSVC; fall back to the
    # always-correct scalar checksum path (see checksum.c).
    add_compile_definitions(MKFF_NO_AVX2_ASM)
else()
    list(APPEND MKFF_CORE_SOURCES simd/checksum_avx2.S)
endif()

add_library(mkff SHARED ${MKFF_CORE_SOURCES})

target_include_directories(mkff
    PUBLIC
        $<BUILD_INTERFACE:${PROJECT_SOURCE_DIR}/include>
        $<INSTALL_INTERFACE:include>
    PRIVATE
        ${PROJECT_SOURCE_DIR}
)

target_compile_definitions(mkff PRIVATE MKFF_BUILDING_SHARED)

if(CMAKE_SYSTEM_NAME STREQUAL "Darwin")
    find_library(COREFOUNDATION_LIBRARY CoreFoundation REQUIRED)
    target_link_libraries(mkff PRIVATE ${COREFOUNDATION_LIBRARY})
elseif(NOT WIN32)
    target_link_libraries(mkff PRIVATE ${CMAKE_DL_LIBS})
endif()

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
