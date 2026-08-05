# Decoder-only FetchContent integration for Ittiam libhevc (Apache-2.0).
# Prefer a local third_party/libhevc checkout when present; otherwise pin
# a known commit. Encoder / examples / fuzzers / gtests are not built.

include(FetchContent)

set(MKFF_LIBHEVC_PINNED_COMMIT "ce30a4b0e333b71bfb9fe90ae349f93fada706c8")

if(EXISTS "${PROJECT_SOURCE_DIR}/third_party/libhevc/CMakeLists.txt")
    set(libhevc_SOURCE_DIR "${PROJECT_SOURCE_DIR}/third_party/libhevc")
    message(STATUS "MKFF: using local third_party/libhevc")
else()
    FetchContent_Declare(
        libhevc
        GIT_REPOSITORY https://github.com/ittiam-systems/libhevc.git
        GIT_TAG        ${MKFF_LIBHEVC_PINNED_COMMIT}
        GIT_SHALLOW    TRUE
    )
    FetchContent_GetProperties(libhevc)
    if(NOT libhevc_POPULATED)
        cmake_policy(SET CMP0169 OLD)
        FetchContent_Populate(libhevc)
    endif()
endif()

set(HEVC_ROOT "${libhevc_SOURCE_DIR}")

if(MSVC)
    # Overlay MSVC-compatible platform macros (GCC attributes/builtins → MSVC).
    file(COPY
        "${PROJECT_SOURCE_DIR}/cmake/libhevc_msvc/ihevc_platform_macros.h"
        DESTINATION "${HEVC_ROOT}/common/x86"
    )
endif()

set(LIBHEVC_COMMON_SRCS "")
set(LIBHEVC_COMMON_ASMS "")
set(LIBHEVCDEC_SRCS "")
set(LIBHEVCDEC_ASMS "")

if(CMAKE_SYSTEM_PROCESSOR MATCHES "^(aarch64|arm64|ARM64)$")
    set(SYSTEM_PROCESSOR "aarch64")
elseif(CMAKE_SYSTEM_PROCESSOR MATCHES "^(arm|ARM|armv7)")
    set(SYSTEM_PROCESSOR "aarch32")
else()
    set(SYSTEM_PROCESSOR "x86_64")
endif()

include("${HEVC_ROOT}/common/common.cmake")
include("${HEVC_ROOT}/decoder/libhevcdec.cmake")

if(MSVC)
    # Drop pthread ithread.c and use our Win32 implementation.
    list(REMOVE_ITEM LIBHEVC_COMMON_SRCS "${HEVC_ROOT}/common/ithread.c")
    # libhevcdec target was already created with the old source list; rebuild it.
    get_target_property(_srcs libhevcdec SOURCES)
    list(REMOVE_ITEM _srcs "${HEVC_ROOT}/common/ithread.c")
    list(APPEND _srcs "${PROJECT_SOURCE_DIR}/cmake/libhevc_msvc/ithread_win.c")
    set_property(TARGET libhevcdec PROPERTY SOURCES ${_srcs})
endif()

if(TARGET libhevcdec)
    if(MSVC)
        set_property(TARGET libhevcdec PROPERTY COMPILE_OPTIONS "")
        target_compile_options(libhevcdec PRIVATE /W0 /wd4018 /wd4244 /wd4267 /wd4146 /wd4305 /wd4311)
        target_compile_definitions(libhevcdec PRIVATE
            X86
            DISABLE_AVX2
            DEFAULT_ARCH=D_ARCH_X86_SSE42
            ENABLE_MAIN_REXT_PROFILE
            _CRT_SECURE_NO_WARNINGS
        )
        target_link_libraries(libhevcdec PUBLIC Threads::Threads)
    else()
        target_compile_definitions(libhevcdec PRIVATE ENABLE_MAIN_REXT_PROFILE)
        if(CMAKE_SYSTEM_NAME STREQUAL "Darwin")
            if(SYSTEM_PROCESSOR STREQUAL "aarch64")
                target_compile_definitions(libhevcdec PRIVATE ARMV8 DARWIN DEFAULT_ARCH=D_ARCH_ARMV8_GENERIC)
            else()
                target_compile_definitions(libhevcdec PRIVATE X86 DARWIN DISABLE_AVX2 DEFAULT_ARCH=D_ARCH_X86_GENERIC)
            endif()
        elseif(SYSTEM_PROCESSOR STREQUAL "aarch64")
            target_compile_definitions(libhevcdec PRIVATE ARMV8 DEFAULT_ARCH=D_ARCH_ARMV8_GENERIC ENABLE_NEON)
        else()
            target_compile_definitions(libhevcdec PRIVATE X86 X86_LINUX=1 DISABLE_AVX2 DEFAULT_ARCH=D_ARCH_X86_SSE42)
            target_compile_options(libhevcdec PRIVATE -msse4.2 -mno-avx)
        endif()
        find_package(Threads REQUIRED)
        target_link_libraries(libhevcdec PUBLIC Threads::Threads)
        if(NOT APPLE)
            target_link_libraries(libhevcdec PUBLIC m)
        endif()
    endif()

    target_include_directories(libhevcdec PUBLIC
        ${HEVC_ROOT}/common
        ${HEVC_ROOT}/decoder
    )
    if(SYSTEM_PROCESSOR STREQUAL "x86_64")
        target_include_directories(libhevcdec PUBLIC
            ${HEVC_ROOT}/common/x86
            ${HEVC_ROOT}/decoder/x86
        )
    endif()

    if(NOT MSVC)
        # already found above
    else()
        find_package(Threads REQUIRED)
    endif()

    set_target_properties(libhevcdec PROPERTIES
        POSITION_INDEPENDENT_CODE ON
        C_VISIBILITY_PRESET hidden
    )
endif()
