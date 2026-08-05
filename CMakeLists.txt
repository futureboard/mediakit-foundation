cmake_minimum_required(VERSION 3.20)

project(mkff
    VERSION 0.1.0
    DESCRIPTION "MediaKit Foundation Framework"
    LANGUAGES C ASM
)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS OFF)

if(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
    set(CMAKE_BUILD_TYPE Debug CACHE STRING "Build type" FORCE)
endif()

set(CMAKE_ARCHIVE_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/lib)
set(CMAKE_LIBRARY_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/lib)
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)

if(NOT CMAKE_SYSTEM_NAME STREQUAL "Linux")
    message(FATAL_ERROR "This milestone only implements the Linux backend.")
endif()

find_package(PkgConfig REQUIRED)
pkg_check_modules(LIBVA REQUIRED IMPORTED_TARGET libva)
pkg_check_modules(LIBVA_DRM REQUIRED IMPORTED_TARGET libva-drm)
pkg_check_modules(LIBDRM REQUIRED IMPORTED_TARGET libdrm)

find_package(Threads REQUIRED)

add_compile_options(-Wall -Wextra)
add_compile_definitions(_GNU_SOURCE)

enable_testing()

add_subdirectory(src/core)
add_subdirectory(src/platform/linux)
add_subdirectory(src/cli)
add_subdirectory(tests)

install(DIRECTORY include/mkff DESTINATION include)
