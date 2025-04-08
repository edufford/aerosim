# AeroSim Linux build expected output

## Expected terminal output for `install_aerosim.sh` in Linux

Choosing option 0, Install All Packages:


```sh

        ___                 _____  _          
       /   | ___  ________ / ___/ (_)___ ___   
      / /| |/ _ \/ ___/ __ \__  \/ / __ `__ \  
     / ___ /  __/ /  / /_/ /__/ / / / / / / /  
    /_/  |_\___/_/   \____/____/_/_/ /_/ /_/   


=============================================
         AeroSim Interactive Installer       
=============================================

Select the components to install:
[0] Install All Packages
[1] AeroSim Development Environment
[2] Unreal Integration
[3] Omniverse Integration
[4] AeroSim Asset Library
[5] Simulink Integration
[6] AeroSim Application
[7] Pixel Streaming
[8] Exit Installation

Enter option numbers separated by space: 0
AEROSIM_UNREAL_ENGINE_ROOT is set to: /home/user/Documents/Dev/UnrealEngineAeroSim

----- Installing AeroSim Development Environment -----
AeroSim repo already exists at 'aerosim/'. Skipping clone.
Updating repository...
AEROSIM_ROOT set to: /home/user/AeroSim/aerosim
AEROSIM_WORLD_LINK_LIB set to: /home/user/AeroSim/aerosim/aerosim-world-link/lib

----- Installing AeroSim Unreal Integration -----
Cloning AeroSim Unreal project repository...
Cloning into 'aerosim-unreal-project'...
remote: Enumerating objects: 41, done.
remote: Counting objects: 100% (41/41), done.
remote: Compressing objects: 100% (33/33), done.
remote: Total 41 (delta 1), reused 41 (delta 1), pack-reused 0 (from 0)
Receiving objects: 100% (41/41), 19.26 KiB | 402.00 KiB/s, done.
Resolving deltas: 100% (1/1), done.
Cloning AeroSim Unreal plugin repository...
Cloning into 'aerosim-unreal-plugin'...
remote: Enumerating objects: 79, done.
remote: Counting objects: 100% (79/79), done.
remote: Compressing objects: 100% (66/66), done.
remote: Total 79 (delta 3), reused 79 (delta 3), pack-reused 0 (from 0)
Receiving objects: 100% (79/79), 73.50 KiB | 1.27 MiB/s, done.
Resolving deltas: 100% (3/3), done.
AEROSIM_UNREAL_PROJECT_ROOT set to: /home/user/AeroSim/aerosim-unreal-project

----- Installing AeroSim Omniverse Integration -----
Cloning AeroSim Omniverse Kit App repository...
Cloning into 'aerosim-omniverse-kit-app'...
remote: Enumerating objects: 65, done.
remote: Counting objects: 100% (65/65), done.
remote: Compressing objects: 100% (53/53), done.
remote: Total 65 (delta 6), reused 65 (delta 6), pack-reused 0 (from 0)
Receiving objects: 100% (65/65), 390.79 KiB | 1.92 MiB/s, done.
Resolving deltas: 100% (6/6), done.
Cloning AeroSim Omniverse extension repository...
Cloning into 'aerosim.omniverse.extension'...
remote: Enumerating objects: 41, done.
remote: Counting objects: 100% (41/41), done.
remote: Compressing objects: 100% (30/30), done.
remote: Total 41 (delta 4), reused 41 (delta 4), pack-reused 0 (from 0)
Receiving objects: 100% (41/41), 186.17 KiB | 958.00 KiB/s, done.
Resolving deltas: 100% (4/4), done.
AEROSIM_OMNIVERSE_ROOT set to: /home/user/AeroSim/aerosim-omniverse-kit-app

----- Installing AeroSim Asset Library -----
Installing Unreal-native assets to: /home/user/AeroSim/aerosim-unreal-project/Plugins
Cloning AeroSim Unreal asset library repository...
Cloning into 'aerosim-assets-unreal'...
remote: Enumerating objects: 509, done.
remote: Counting objects: 100% (509/509), done.
remote: Compressing objects: 100% (506/506), done.
remote: Total 509 (delta 0), reused 509 (delta 0), pack-reused 0 (from 0)
Receiving objects: 100% (509/509), 71.45 KiB | 717.00 KiB/s, done.
Filtering content: 100% (445/445), 711.21 MiB | 5.16 MiB/s, done.
AEROSIM_ASSETS_UNREAL_ROOT set to: /home/user/AeroSim/aerosim-unreal-project/Plugins/aerosim-assets-unreal

Installing USD assets to: /home/user/AeroSim/aerosim-assets
Cloning AeroSim USD asset library repository...
Cloning into 'aerosim-assets'...
remote: Enumerating objects: 68, done.
remote: Counting objects: 100% (68/68), done.
remote: Compressing objects: 100% (55/55), done.
remote: Total 68 (delta 3), reused 68 (delta 3), pack-reused 0 (from 0)
Receiving objects: 100% (68/68), 9.73 KiB | 9.73 MiB/s, done.
Resolving deltas: 100% (3/3), done.
Filtering content: 100% (75/75), 747.13 MiB | 9.98 MiB/s, done.
AEROSIM_ASSETS_ROOT set to: /home/user/AeroSim/aerosim-assets

----- Installing AeroSim Simulink Integration -----
Cloning AeroSim Simulink repository...
Cloning into 'aerosim-simulink'...
remote: Enumerating objects: 40, done.
remote: Counting objects: 100% (40/40), done.
remote: Compressing objects: 100% (34/34), done.
remote: Total 40 (delta 4), reused 40 (delta 4), pack-reused 0 (from 0)
Receiving objects: 100% (40/40), 562.82 KiB | 2.00 MiB/s, done.
Resolving deltas: 100% (4/4), done.
AEROSIM_SIMULINK_ROOT set to: /home/user/AeroSim/aerosim-simulink

----- Installing AeroSim Application -----
Cloning AeroSim app repository...
Cloning into 'aerosim-app'...
remote: Enumerating objects: 59, done.
remote: Counting objects: 100% (59/59), done.
remote: Compressing objects: 100% (53/53), done.
remote: Total 59 (delta 2), reused 59 (delta 2), pack-reused 0 (from 0)
Receiving objects: 100% (59/59), 93.74 KiB | 914.00 KiB/s, done.
Resolving deltas: 100% (2/2), done.
Installing app dependencies...
bun install v1.2.3 (8c4d3ff8)

$ electron-builder install-app-deps
  • electron-builder  version=24.13.3
  • loaded configuration  file=package.json ("build" field)

+ @types/node@20.17.24
+ @types/react@19.0.11
+ @types/react-dom@19.0.4
+ @typescript-eslint/eslint-plugin@6.21.0
+ @typescript-eslint/parser@6.21.0
+ @vitejs/plugin-react@4.3.4
+ autoprefixer@10.4.21
+ concurrently@9.1.2
+ electron@28.3.3
+ electron-builder@24.13.3
+ eslint@8.57.1
+ eslint-plugin-react-hooks@4.6.2
+ eslint-plugin-react-refresh@0.4.19
+ postcss@8.5.3
+ prettier@3.5.3
+ tailwindcss@3.4.17
+ typescript@5.8.2
+ vite@5.4.14
+ vite-plugin-electron@0.29.0
+ vite-plugin-electron-renderer@0.14.6
+ @epicgames-ps/lib-pixelstreamingfrontend-ue5.3@1.0.1
+ @nvidia/omniverse-webrtc-streaming-library@4.4.2
+ @radix-ui/react-dialog@1.1.6
+ @radix-ui/react-separator@1.1.2
+ @radix-ui/react-slot@1.1.2
+ class-variance-authority@0.7.1
+ clsx@2.1.1
+ cross-env@7.0.3
+ electron-updater@6.3.9
+ lucide-react@0.474.0
+ react@18.2.0
+ react-dom@18.2.0
+ react-rnd@10.5.2
+ shadcn-ui@0.8.0
+ tailwind-merge@3.0.2
+ tailwindcss-animate@1.0.7
+ wait-on@7.2.0
+ zustand@4.5.6

632 packages installed [2.91s]
AEROSIM_APP_ROOT set to: /home/user/AeroSim/aerosim-app
AEROSIM_APP_WS_PORT set to: 5001

----- Installing Pixel Streaming -----
TODO...
Pixel Streaming dependencies installed.

----- Installation Complete -----
The following AeroSim environment variables have been set:
--------------------------------------------
export AEROSIM_ROOT=/home/user/AeroSim/aerosim
export AEROSIM_WORLD_LINK_LIB=/home/user/AeroSim/aerosim/aerosim-world-link/lib
export AEROSIM_UNREAL_PROJECT_ROOT=/home/user/AeroSim/aerosim-unreal-project
export AEROSIM_OMNIVERSE_ROOT=/home/user/AeroSim/aerosim-omniverse-kit-app
export AEROSIM_ASSETS_UNREAL_ROOT=/home/user/AeroSim/aerosim-unreal-project/Plugins/aerosim-assets-unreal
export AEROSIM_ASSETS_ROOT=/home/user/AeroSim/aerosim-assets
export AEROSIM_SIMULINK_ROOT=/home/user/AeroSim/aerosim-simulink
export AEROSIM_APP_ROOT=/home/user/AeroSim/aerosim-app
export AEROSIM_APP_WS_PORT=5001
--------------------------------------------

IMPORTANT: Add the above exports to your shell's rc file (e.g. ~/.bashrc or ~/.zshrc) to persist them.

```
