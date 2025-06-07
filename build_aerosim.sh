#!/bin/bash

# Exit on error
set -e

# Install/sync the Python virtual environment
uv sync --no-build --no-install-workspace

# Activate the Python virtual environment
export PATH=$PATH:$PWD/.venv/bin/
alias python=./.venv/bin/python
alias python3=./.venv/bin/python3

# Build AeroSim with force flag to ensure aerosim-world-link is always rebuilt
uv run --no-project build.py -f $*
