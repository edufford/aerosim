#!/usr/bin/env python
"""Build script for coordinating UV Python package management with maturin Rust builds.

This script ensures proper sequencing of dependencies and compilation across the project.
"""

import os
import subprocess
import sys
import argparse
import shutil
import glob
from pathlib import Path

def run_command(cmd, cwd=None, verbose=False):
    """Run a command and return its output."""
    print(f"Running: {' '.join(cmd if isinstance(cmd, list) else [cmd])}")
    try:
        if verbose:
            # Run with live output for verbose mode
            process = subprocess.Popen(
                cmd,
                cwd=cwd,
                text=True,
                encoding='utf-8',
                errors='replace'
            )
            process.communicate()
            if process.returncode != 0:
                print(f"Command failed with return code {process.returncode}")
                sys.exit(1)
            return ""
        else:
            # Capture output for normal mode
            result = subprocess.run(
                cmd,
                cwd=cwd,
                check=True,
                text=True,
                capture_output=True,
                encoding='utf-8',  # Explicitly set encoding to utf-8
                errors='replace'   # Replace any invalid characters
            )
            return result.stdout
    except subprocess.CalledProcessError as e:
        print(f"Error running command: {e}")
        print(f"STDOUT: {e.stdout}")
        print(f"STDERR: {e.stderr}")
        sys.exit(1)

def clean_build_artifacts(project_root, verbose=False):
    """Clean up build artifacts."""
    print("Cleaning build artifacts...")

    # Clean Rust build artifacts
    run_command(["cargo", "clean"], cwd=project_root, verbose=verbose)

    # Clean Python build artifacts
    dist_dir = project_root / "dist"
    if dist_dir.exists():
        print(f"Removing {dist_dir}")
        shutil.rmtree(dist_dir)

    # Clean *.pyd built Rust library artifacts (Windows) from Python package folders
    # because maturin has a bug that can't replace them while building the wheels.
    pyd_files = glob.glob(os.path.join(project_root, '**', '*.pyd'), recursive=True)
    for file_path in pyd_files:
        try:
            os.remove(file_path)
            print(f"Deleted: {file_path}")
        except OSError as e:
            print(f"Error deleting {file_path}: {e}")

    # Clean aerosim-world-link artifacts
    world_link_dir = project_root / "aerosim-world-link"
    world_link_lib_dir = world_link_dir / "lib"
    if world_link_lib_dir.exists():
        print(f"Removing {world_link_lib_dir}")
        shutil.rmtree(world_link_lib_dir)

    # Clean target directories in all packages
    packages = [
        "aerosim-controllers",
        "aerosim-core",
        "aerosim-data",
        "aerosim-dynamics-models",
        "aerosim-scenarios",
        "aerosim-sensors",
        "aerosim-world",
        "aerosim-world-link"
    ]

    for package in packages:
        package_target = project_root / package / "target"
        if package_target.exists():
            print(f"Removing {package_target}")
            shutil.rmtree(package_target)

    print("Clean completed successfully!")

def main():
    """Main build function."""
    # Parse command line arguments
    parser = argparse.ArgumentParser(description='Build AeroSim components')
    parser.add_argument('-v', '--verbose', action='store_true', help='Enable verbose output')
    parser.add_argument('-c', '--clean', action='store_true', help='Clean up build artifacts before building')
    parser.add_argument('--wheel', action='store_true', help='Build wheel packages for distribution')

    args = parser.parse_args()

    project_root = Path(__file__).parent.absolute()

    # Clean build artifacts if requested
    if args.clean or args.wheel:
        clean_build_artifacts(project_root, args.verbose)

    # Ensure UV environment is set up
    if not (project_root / ".venv").exists():
        print("Setting up UV .venv virtual environment...")
        run_command(["uv", "sync", "--no-build", "--no-install-workspace"], cwd=project_root, verbose=args.verbose)

    if args.wheel:
        print("Building Python wheels to the 'dist/' folder for distribution...")
        run_command(["uv", "build", "--all-packages", "--wheel"], cwd=project_root, verbose=args.verbose)
    else:
        # Step 1: Build all Rust crates using maturin
        print("Building Rust crates with maturin...")

        # Path to maturin in the virtual environment
        venv_path = project_root / ".venv"
        if sys.platform == "win32":
            maturin_path = venv_path / "Scripts" / "maturin.exe"
        else:
            maturin_path = venv_path / "bin" / "maturin"

        if not maturin_path.exists():
            print(f"Maturin not found at {maturin_path}. Installing...")
            run_command(["uv", "run", "pip", "install", "maturin>=1.5,<2.0"], cwd=project_root, verbose=args.verbose)

        # Build each package individually
        packages = [
            "aerosim-controllers",
            "aerosim-core",
            "aerosim-data",
            "aerosim-dynamics-models",
            "aerosim-scenarios",
            "aerosim-sensors",
            "aerosim-world"
        ]

        # Build dev packages for .venv local virtual environment
        total_packages = len(packages)
        for i, package in enumerate(packages):
            print(f"Building package {i+1}/{total_packages} {package}...")
            package_path = project_root / package / "Cargo.toml"
            if package_path.exists():
                run_command(
                    [str(maturin_path), "develop", "--release", "--skip-install", "-m", str(package_path)],
                    cwd=project_root, verbose=args.verbose
                )
            else:
                print(f"Warning: {package_path} does not exist, skipping")

        # Step 2: Build aerosim-world-link explicitly
        world_link_dir = project_root / "aerosim-world-link"
        print("Building aerosim-world-link...")

        # Ensure lib directory exists
        (world_link_dir / "lib").mkdir(exist_ok=True)

        # Always build aerosim-world-link regardless of existing files
        if sys.platform == "win32":
            bat_file = world_link_dir / "build.bat"
            if bat_file.exists():
                print(f"Running batch file: {bat_file}")
                run_command(f"cmd /c {bat_file}", cwd=world_link_dir, verbose=args.verbose)
            else:
                print(f"Warning: build.bat not found at {bat_file}")
                print("Files in directory:")
                for file in world_link_dir.iterdir():
                    print(f"  {file}")
        else:
            # Make the script executable
            shell_file = world_link_dir / "build.sh"
            if shell_file.exists():
                run_command(["chmod", "+x", str(shell_file)], cwd=world_link_dir, verbose=args.verbose)
                run_command(["./build.sh"], cwd=world_link_dir, verbose=args.verbose)
            else:
                print(f"Warning: build.sh not found at {shell_file}")
                print("Files in directory:")
                for file in world_link_dir.iterdir():
                    print(f"  {file}")

        # Step 3: Install final built dev packages to the Python virtual environment
        print("Installing final Python packages to the UV .venv virtual environment...")
        run_command(
            ["uv", "sync"],
            cwd=project_root, verbose=args.verbose
        )

    print("Build completed successfully!")

if __name__ == "__main__":
    main()
