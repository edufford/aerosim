@echo off

@REM Install/sync only the project dependencies to the Python virtual environment
uv sync --no-build --no-install-workspace || exit /b %ERRORLEVEL%

@REM Activate the Python virtual environment
call .venv\Scripts\activate

@REM Build AeroSim with force flag to ensure aerosim-world-link is always rebuilt
uv run --no-project build.py -f %*
