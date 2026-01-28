#!/bin/bash
# Build script for Template FMU

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CPP_DIR="${SCRIPT_DIR}/cpp/template_fmu"
BUILD_DIR="${CPP_DIR}/build"

echo "Building Template FMU..."

mkdir -p "${BUILD_DIR}"
cd "${BUILD_DIR}"

cmake .. -DCMAKE_BUILD_TYPE=Release
cmake --build . --config Release

if [ -f "${BUILD_DIR}/template_fmu.fmu" ]; then
    cp "${BUILD_DIR}/template_fmu.fmu" "${SCRIPT_DIR}/../examples/fmu/"
    echo "Built and copied template_fmu.fmu to ../examples/fmu"
else
    echo "Build failed - FMU file not found"
    exit 1
fi
