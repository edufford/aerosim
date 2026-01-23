cmake_minimum_required(VERSION 3.14)
project(evtol_effectors_fmu_model_cpp VERSION 1.0.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)

# FMU model name
set(FMU_MODEL_NAME "evtol_effectors_fmu_model_cpp")

# Fetch CPPFMU (FMI 3.0 version) from PythonFMU3 repository
include(FetchContent)
FetchContent_Declare(
    pythonfmu3
    GIT_REPOSITORY https://github.com/stephensmith25/PythonFMU3.git
    GIT_TAG        master
)
# Only populate, don't configure/build their CMake
FetchContent_GetProperties(pythonfmu3)
if(NOT pythonfmu3_POPULATED)
    FetchContent_Populate(pythonfmu3)
endif()

set(CPPFMU_SOURCE_DIR "${pythonfmu3_SOURCE_DIR}/pythonfmu3/pythonfmu-export/src")
message(STATUS "Using CPPFMU sources from: ${CPPFMU_SOURCE_DIR}")

# Platform-specific settings
if(WIN32)
    set(FMU_PLATFORM "x86_64-windows")
    set(FMU_LIB_EXT ".dll")
elseif(APPLE)
    set(FMU_PLATFORM "x86_64-darwin")
    set(FMU_LIB_EXT ".dylib")
else()
    set(FMU_PLATFORM "x86_64-linux")
    set(FMU_LIB_EXT ".so")
endif()

# Create the shared library
add_library(${FMU_MODEL_NAME} SHARED
    evtol_effectors_fmu.cpp
    ${CPPFMU_SOURCE_DIR}/cppfmu/cppfmu_cs.cpp
    ${CPPFMU_SOURCE_DIR}/cppfmu/fmi_functions.cpp
)

# Include directories
target_include_directories(${FMU_MODEL_NAME} PRIVATE
    ${CPPFMU_SOURCE_DIR}
)

# Output directory for the built library
set_target_properties(${FMU_MODEL_NAME} PROPERTIES
    LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/binaries/${FMU_PLATFORM}"
    RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/binaries/${FMU_PLATFORM}"
    PREFIX ""
)

# Custom target to create the FMU package
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
