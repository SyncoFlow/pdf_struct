cmake_minimum_required(VERSION 3.10)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

set(CMAKE_TOOLCHAIN_FILE "${CMAKE_SOURCE_DIR}/vcpkg/scripts/buildsystems/vcpkg.cmake"
    CACHE STRING "Vcpkg toolchain file")

if(NOT EXISTS "${CMAKE_TOOLCHAIN_FILE}")
    message(FATAL_ERROR "vcpkg toolchain file not found at ${CMAKE_TOOLCHAIN_FILE}")
endif()

project("binding")

find_path(MUPDF_INCLUDE_DIR fitz.h
    PATHS
    "${CMAKE_BINARY_DIR}/vcpkg_installed/x64-windows/include"
    "${CMAKE_SOURCE_DIR}/build/vcpkg_installed/x64-windows/include"
    "$ENV{VCPKG_ROOT}/installed/x64-windows/include"
    PATH_SUFFIXES mupdf
)

find_library(MUPDF_LIBRARY NAMES libmupdf mupdf
    PATHS
    "${CMAKE_BINARY_DIR}/vcpkg_installed/x64-windows/lib"
    "${CMAKE_SOURCE_DIR}/build/vcpkg_installed/x64-windows/lib"
    "$ENV{VCPKG_ROOT}/installed/x64-windows/lib"
)

if(NOT MUPDF_INCLUDE_DIR OR NOT MUPDF_LIBRARY)
    message(FATAL_ERROR "MuPDF not found")
endif()

file(GLOB_RECURSE ALL_CPP_FILES "${CMAKE_CURRENT_SOURCE_DIR}/src_cpp/*.cpp")
add_library(binding STATIC ${ALL_CPP_FILES})

get_filename_component(MUPDF_INCLUDE_PARENT ${MUPDF_INCLUDE_DIR} DIRECTORY)
target_include_directories(binding PUBLIC
    ${MUPDF_INCLUDE_PARENT}
    "${CMAKE_BINARY_DIR}/vcpkg_installed/x64-windows/include"
    "${CMAKE_SOURCE_DIR}/build/vcpkg_installed/x64-windows/include"
    "$ENV{VCPKG_ROOT}/installed/x64-windows/include"
    "${CMAKE_SOURCE_DIR}"
)

target_link_libraries(binding PUBLIC ${MUPDF_LIBRARY})

install(TARGETS binding
    ARCHIVE DESTINATION lib
    LIBRARY DESTINATION lib
    RUNTIME DESTINATION bin)
