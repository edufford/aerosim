# Template FMU CMake Build Configuration
#
# Uses the FMI 3.0 C API directly with headers from the official FMI standard:
# https://github.com/modelica/fmi-standard
#
# To create a new FMU from this template:
# 1. Copy this directory and rename it
# 2. Update FMU_MODEL_NAME below to match your new FMU name
# 3. Update the source file name in add_library() if you renamed it
# 4. Run the build script or: mkdir build && cd build && cmake .. && cmake --build .

cmake_minimum_required(VERSION 3.14)
project(template_fmu VERSION 1.0.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)

# =============================================================================
# FMU Configuration - UPDATE THIS FOR YOUR FMU
# =============================================================================
set(FMU_MODEL_NAME "template_fmu")

# Source files: FMU implementation + reusable FMI 3.0 CS stubs
set(FMU_SOURCES
    template_fmu.cpp
    fmi3_cs_stubs.cpp
)

# =============================================================================
# Fetch FMI 3.0 Headers from Official FMI Standard Repository
# =============================================================================
include(FetchContent)

if(POLICY CMP0169)
    cmake_policy(SET CMP0169 NEW)
endif()

FetchContent_Declare(
    fmi3_headers
    GIT_REPOSITORY https://github.com/modelica/fmi-standard.git
    GIT_TAG        v3.0.2
    EXCLUDE_FROM_ALL
)
FetchContent_MakeAvailable(fmi3_headers)

set(FMI3_INCLUDE_DIR "${fmi3_headers_SOURCE_DIR}/headers")
message(STATUS "Using FMI 3.0 headers from: ${FMI3_INCLUDE_DIR}")

# =============================================================================
# Platform Detection
# =============================================================================
if(WIN32)
    set(FMU_PLATFORM "x86_64-windows")
elseif(APPLE)
    set(FMU_PLATFORM "x86_64-darwin")
else()
    set(FMU_PLATFORM "x86_64-linux")
endif()

# =============================================================================
# Build the FMU Shared Library
# =============================================================================
add_library(${FMU_MODEL_NAME} SHARED
    ${FMU_SOURCES}
)

target_include_directories(${FMU_MODEL_NAME} PRIVATE
    ${FMI3_INCLUDE_DIR}
)

set_target_properties(${FMU_MODEL_NAME} PROPERTIES
    LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/binaries/${FMU_PLATFORM}"
    RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/binaries/${FMU_PLATFORM}"
    PREFIX ""
)

# =============================================================================
# Package the FMU
# =============================================================================
set(FMU_STAGING_DIR "${CMAKE_BINARY_DIR}/fmu_staging")
set(FMU_OUTPUT_FILE "${CMAKE_BINARY_DIR}/${FMU_MODEL_NAME}.fmu")

add_custom_command(
    OUTPUT ${FMU_OUTPUT_FILE}
    COMMAND ${CMAKE_COMMAND} -E remove_directory ${FMU_STAGING_DIR}
    COMMAND ${CMAKE_COMMAND} -E make_directory ${FMU_STAGING_DIR}
    COMMAND ${CMAKE_COMMAND} -E make_directory ${FMU_STAGING_DIR}/binaries/${FMU_PLATFORM}
    COMMAND ${CMAKE_COMMAND} -E copy ${CMAKE_SOURCE_DIR}/modelDescription.xml ${FMU_STAGING_DIR}/
    COMMAND ${CMAKE_COMMAND} -E copy $<TARGET_FILE:${FMU_MODEL_NAME}> ${FMU_STAGING_DIR}/binaries/${FMU_PLATFORM}/
    COMMAND ${CMAKE_COMMAND} -E chdir ${FMU_STAGING_DIR} ${CMAKE_COMMAND} -E tar cf ${FMU_OUTPUT_FILE} --format=zip .
    DEPENDS ${FMU_MODEL_NAME} ${CMAKE_SOURCE_DIR}/modelDescription.xml
    COMMENT "Creating FMU package: ${FMU_OUTPUT_FILE}"
)

add_custom_target(fmu ALL DEPENDS ${FMU_OUTPUT_FILE})
