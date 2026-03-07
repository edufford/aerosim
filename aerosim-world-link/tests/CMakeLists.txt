cmake_minimum_required(VERSION 3.16)
project(aerosim_world_link_tests LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Path to the pre-built aerosim-world-link shared library and header
set(WORLD_LINK_LIB_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../lib")

# Import the shared library
add_library(aerosim_world_link SHARED IMPORTED)
set_target_properties(aerosim_world_link PROPERTIES
    INTERFACE_INCLUDE_DIRECTORIES "${WORLD_LINK_LIB_DIR}"
)
if(WIN32)
    set_target_properties(aerosim_world_link PROPERTIES
        IMPORTED_LOCATION "${WORLD_LINK_LIB_DIR}/aerosim_world_link.dll"
        IMPORTED_IMPLIB "${WORLD_LINK_LIB_DIR}/aerosim_world_link.lib"
    )
else()
    set_target_properties(aerosim_world_link PROPERTIES
        IMPORTED_LOCATION "${WORLD_LINK_LIB_DIR}/libaerosim_world_link.so"
    )
endif()

# Fetch Google Test
include(FetchContent)
FetchContent_Declare(googletest
    GIT_REPOSITORY https://github.com/google/googletest.git
    GIT_TAG v1.17.0
)
FetchContent_MakeAvailable(googletest)

# Test executable
add_executable(test_world_link test_world_link.cpp)
target_link_libraries(test_world_link PRIVATE aerosim_world_link GTest::gtest_main)

if(WIN32)
    # Copy the DLL next to the test binary so it can be found at runtime
    add_custom_command(TARGET test_world_link POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy_if_different
            "${WORLD_LINK_LIB_DIR}/aerosim_world_link.dll"
            $<TARGET_FILE_DIR:test_world_link>
    )
else()
    # Set RPATH so the test binary finds the shared library at runtime
    set_target_properties(test_world_link PROPERTIES
        BUILD_RPATH "${WORLD_LINK_LIB_DIR}"
        INSTALL_RPATH "${WORLD_LINK_LIB_DIR}"
    )
endif()

# Discover individual tests for CTest
enable_testing()
include(GoogleTest)
gtest_discover_tests(test_world_link)
