#!/bin/bash

# Build script for C++ eVTOL Effectors FMU

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CPP_DIR="${SCRIPT_DIR}/cpp/evtol_effectors"
BUILD_DIR="${CPP_DIR}/build"

echo "Building C++ eVTOL Effectors FMU..."

# Create build directory
mkdir -p "${BUILD_DIR}"
cd "${BUILD_DIR}"

# Configure with CMake
cmake .. -DCMAKE_BUILD_TYPE=Release

# Build
cmake --build . --config Release

# Check if build was successful
if [ -f "${BUILD_DIR}/evtol_effectors_fmu_model_cpp.fmu" ]; then
    # Move FMU to examples directory
    cp "${BUILD_DIR}/evtol_effectors_fmu_model_cpp.fmu" "${SCRIPT_DIR}/../examples/fmu/"
    echo "Built and copied evtol_effectors_fmu_model_cpp.fmu to ../examples/fmu"
else
    echo "Build failed - FMU file not found"
    exit 1
fi
