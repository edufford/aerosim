# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
# Full development build (builds all Rust crates via maturin + aerosim-world-link + installs to .venv)
./build_aerosim.sh

# Under the hood: uv sync --no-build && uv run build.py
# Build flags: -v (verbose), -c (clean first), --wheel (production wheels)

# Build production wheels to dist/
./build_wheels.sh
```

The build process (in `build.py`) does three steps:
1. Builds each Rust crate with `maturin develop --release --skip-install`
2. Builds `aerosim-world-link` (C++ FFI bridge via `aerosim-world-link/build.sh`)
3. Installs everything into the UV .venv with `uv sync`

## Testing

```bash
# Rust tests for a specific crate
cd aerosim-sensors && cargo test tests --verbose

# Python tests
python -m pytest aerosim-core/tests/
python -m pytest aerosim-sensors/tests/

# Single test file
python -m pytest aerosim-core/tests/test_quaternion.py
```

CI runs `cargo test tests --verbose` for each crate: aerosim-controllers, aerosim-data, aerosim-dynamics-models, aerosim-scenarios, aerosim-sensors, aerosim-world, aerosim-world-link.

## Linting

```bash
python -m ruff check
cargo check
```

## Architecture

AeroSim is a modular aerospace simulation platform combining Rust (performance-critical core) with Python (orchestration API) via PyO3/maturin bindings.

### Rust Workspace Crates

| Crate | Purpose |
|---|---|
| `aerosim-core` | Coordinate systems (LLA/NED/Cartesian), actor definitions, trajectory generation |
| `aerosim-data` | Message types, middleware abstractions (Kafka, Zenoh, DDS), serializers |
| `aerosim-world` | Orchestrator, scene graph (Bevy ECS), FMU driver, simulation clock, MCAP data recording |
| `aerosim-controllers` | Flight control systems |
| `aerosim-dynamics-models` | Flight dynamics models (FMU integration) |
| `aerosim-scenarios` | Scenario definition and config translation |
| `aerosim-sensors` | Sensor simulation (GNSS, IMU, ADS-B, camera) |
| `aerosim-macros` | Procedural macros for `AerosimMessage` derive (not exported to Python) |
| `aerosim-world-link` | Shared library for external renderers/sim engines (Unreal plugin, Omniverse extension) to connect to the aerosim simulation and scene graph data (separate build, not in Cargo workspace) |

Each Rust crate (except `aerosim-macros` and `aerosim-world-link`) is also a Python package via PyO3, exposing a `_aerosim_<name>` module.

### Python Package (`aerosim/`)

- `aerosim.core` — `AeroSim` class (main simulation API), `SimConfig`
- `aerosim.io.websockets` — WebSocket servers for UI streaming (camera images, flight data)
- `aerosim.io.input` — Input handlers (keyboard, gamepad)
- `aerosim.visualization` — Camera and visualization managers

### Key Data Flow

```
SimClock (tick) → FMU Driver (dynamics) → VehicleState → SceneGraph (Bevy ECS)
    → Renderer (Unreal/Omniverse) → Sensor data (camera, IMU, GNSS, ADS-B)
    → DataManager (MCAP recording) + WebSocket streaming
```

The simulation is driven by **tick groups** configured in the sim config JSON. The SimClock publishes a clock topic (e.g. `aerosim.clock.tick_group_01`) to trigger each tick group. After publishing, the orchestrator waits for all configured **sync topics** (e.g. `aerosim.actor1.vehicle_state`) in that group to be published before advancing to the next tick group. This ensures deterministic ordering — downstream components (FMU drivers) must complete their work and publish results before the simulation proceeds. Renderers are not part of tick groups; they are triggered by the publishing of scene graph updates.

Components communicate via **message-passing middleware** (Kafka by default, Zenoh also supported) using pub/sub topics like `aerosim.clock.*`, `aerosim.actor*.*`, `aerosim.renderer.*`. The middleware feature flags in `aerosim-data/Cargo.toml` control which transports are compiled (default: `kafka` + `zenoh`).

### Simulation Entry Point

```python
from aerosim import AeroSim
sim = AeroSim(enable_websockets=True)
sim.run("sim_config.json", sim_config_dir="examples/config")
```

The JSON config defines actors, FMU models, renderers, tick groups, and topic mappings. The `Orchestrator` (Rust) manages the simulation loop; `FmuDriver` (Rust) handles FMI 3.0 dynamics models.

### External Renderers

The simulation connects to separate renderer processes via middleware:
- **Unreal Engine** — `aerosim-unreal-project` repository
- **NVIDIA Omniverse Kit** — `aerosim-omniverse-kit-app` repository

Launch with: `./launch_aerosim.sh --unreal` or `./launch_aerosim.sh --omniverse`

### Git and PR Workflow

- Always create branches with a `claude/` prefix (e.g. `claude/fix-camera-stream`, `claude/add-sensor-type`)
- Always write PR descriptions in raw markdown format (not rendered/rich text)

### Key Conventions

- Rust edition 2021, resolver v2, PyO3 with abi3-py39 for broad Python compatibility
- Python ≥ 3.12 required
- `uv` for Python package management (not pip/rye directly)
- All message types derive `AerosimMessage` (custom proc macro) + `serde::Serialize/Deserialize` + PyO3 `#[pyclass]`
- Workspace dependencies are centralized in the root `Cargo.toml`
