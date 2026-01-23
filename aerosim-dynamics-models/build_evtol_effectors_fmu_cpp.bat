@echo off
REM Build script for C++ eVTOL Effectors FMU (Windows)

set SCRIPT_DIR=%~dp0
set CPP_DIR=%SCRIPT_DIR%cpp\evtol_effectors
set BUILD_DIR=%CPP_DIR%\build

echo Building C++ eVTOL Effectors FMU...

REM Create build directory
if not exist "%BUILD_DIR%" mkdir "%BUILD_DIR%"
cd /d "%BUILD_DIR%"

REM Configure with CMake
cmake .. -DCMAKE_BUILD_TYPE=Release

REM Build
cmake --build . --config Release

REM Check if build was successful
if exist "%BUILD_DIR%\evtol_effectors_fmu_model_cpp.fmu" (
    copy "%BUILD_DIR%\evtol_effectors_fmu_model_cpp.fmu" "%SCRIPT_DIR%..\examples\fmu\"
    echo Built and copied evtol_effectors_fmu_model_cpp.fmu to ..\examples\fmu
) else (
    echo Build failed - FMU file not found
    exit /b 1
)
