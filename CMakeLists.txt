cmake_minimum_required(VERSION 3.20)

# GNU-syntax (AT&T) assembly (src/core/simd/checksum_avx2.S) only
# assembles with GCC/Clang's integrated assembler; MSVC would need a
# MASM translation, which doesn't exist yet, so the ASM language is
# only enabled where it's actually usable. checksum.c falls back to the
# always-correct scalar path wherever MKFF_NO_AVX2_ASM is defined.
if(WIN32)
    project(mkff VERSION 0.1.0 DESCRIPTION "MediaKit Foundation Framework" LANGUAGES C)
else()
    project(mkff VERSION 0.1.0 DESCRIPTION "MediaKit Foundation Framework" LANGUAGES C ASM)
endif()

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS OFF)

if(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
    set(CMAKE_BUILD_TYPE Debug CACHE STRING "Build type" FORCE)
endif()

set(CMAKE_ARCHIVE_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/lib)
set(CMAKE_LIBRARY_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/lib)
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)

if(NOT (CMAKE_SYSTEM_NAME STREQUAL "Linux" OR CMAKE_SYSTEM_NAME STREQUAL "Darwin" OR CMAKE_SYSTEM_NAME STREQUAL "Windows"))
    message(FATAL_ERROR "mkff has platform backends for Linux, macOS (Darwin), and Windows only.")
endif()

if(CMAKE_SYSTEM_NAME STREQUAL "Linux")
    find_package(PkgConfig REQUIRED)
    pkg_check_modules(LIBVA REQUIRED IMPORTED_TARGET libva)
    pkg_check_modules(LIBVA_DRM REQUIRED IMPORTED_TARGET libva-drm)
    pkg_check_modules(LIBDRM REQUIRED IMPORTED_TARGET libdrm)
    find_package(Threads REQUIRED)
endif()

if(MSVC)
    add_compile_options(/W4)
else()
    add_compile_options(-Wall -Wextra)
endif()

if(NOT WIN32)
    add_compile_definitions(_GNU_SOURCE)
endif()

enable_testing()

add_subdirectory(src/codecs)
add_subdirectory(src/core)

if(CMAKE_SYSTEM_NAME STREQUAL "Linux")
    add_subdirectory(src/platform/linux)
elseif(CMAKE_SYSTEM_NAME STREQUAL "Darwin")
    add_subdirectory(src/platform/macos)
elseif(CMAKE_SYSTEM_NAME STREQUAL "Windows")
    add_subdirectory(src/platform/windows)
endif()

add_subdirectory(src/cli)
add_subdirectory(tests)

install(DIRECTORY include/mkff DESTINATION include)
