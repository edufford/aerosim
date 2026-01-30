@echo off
REM Build script for Template FMU

set SCRIPT_DIR=%~dp0
set CPP_DIR=%SCRIPT_DIR%cpp\template_fmu
set BUILD_DIR=%CPP_DIR%\build

echo Building Template FMU...

if not exist "%BUILD_DIR%" mkdir "%BUILD_DIR%"
pushd "%BUILD_DIR%"

cmake ..
if %ERRORLEVEL% neq 0 (
    echo CMake configuration failed
    popd
    exit /b 1
)

cmake --build . --config Release
if %ERRORLEVEL% neq 0 (
    echo CMake build failed
    popd
    exit /b 1
)

if exist "%BUILD_DIR%\template_fmu.fmu" (
    copy "%BUILD_DIR%\template_fmu.fmu" "%SCRIPT_DIR%..\examples\fmu\" >nul
    echo Built and copied template_fmu.fmu to ..\examples\fmu
    popd
) else (
    echo Build failed - FMU file not found
    popd
    exit /b 1
)
