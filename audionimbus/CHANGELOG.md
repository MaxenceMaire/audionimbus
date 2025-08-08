# Changelog

## [0.7.2] - 2025-08-08

### Fixed

- `mix` on `AudioBuffer` would incorrectly mix `self` into `other` instead of the other way around.
- `downmix` on `AudioBuffer` would take a mutable `self`, which was unnecessary.

## [0.7.1] - 2025-06-28

### Fixed

- The FMOD integration uses `SimulationSettings` but it was not made public.

## [0.7.0] - 2025-06-28

### Changed

- Make `OpenClDeviceSettings` fields public.

### Fixed

- Calling `run_reflections` (resp. `run_direct`, `run_pathing`) on a `Simulator` that was not configured with the appropriate reflections (resp., direct, pathing) settings would result in a segmentation fault.

## [0.6.4] - 2025-04-16

### Fixed

- Dependency `audionimbus-sys` would fail to compile on some platforms when feature `fmod` was enabled, due to missing flags. It has been updated to version `4.6.2-fmodwwise.2`.

## [0.6.3] - 2025-04-15

### Added

- Support for the Steam Audio Wwise integration.

## [0.6.2] - 2025-04-14

### Added

- Documentation generation for feature `fmod`.

### Fixed

- Version `4.6.2-fmod.2` of dependency `audionimbus-sys` would fail to compile when feature `fmod` was enabled. It has been updated to version `4.6.2-fmod.3`.

## [0.6.1] - 2025-04-13

### Fixed

- Version `4.6.2-fmod` of dependency `audionimbus-sys` would prevent the documentation from being generated. It has been updated to version `4.6.2-fmod.1`.

## [0.6.0] - 2025-04-13

### Added

- Support for the Steam Audio FMOD integration.

## [0.5.1] - 2025-04-12

### Added

- Support for Steam Audio v.4.6.1 via `audionimbus-sys` v.4.6.1.

## [0.5.0] - 2025-04-10

### Fixed

- Casting `u32` flags into `std::os::raw::c_uint` would fail on systems expecting `std::os::raw::c_int` IPL flags.

## [0.4.0] - 2025-04-01

### Added

- Implement `mix`, `downmix`, `convert_ambisonics`, `convert_ambisonics_into` methods for `AudioBuffer`.
- Implement `tail`, `tail_size` methods for `PanningEffect`, `PathEffect`, `AmbisonicsBinauralEffect`, `AmbisonicsDecodeEffect`, `AmbisonicsDecodeEffect`, `AmbisonicsPanningEffect`, `AmbisonicsRotationEffect`, `BinauralEffect`, `DirectEffect`, `DirectEffect`, `PathEffect`, `ReflectionEffect`, `VirtualSurroundEffect`.
- Implement `tail_into_mixer` for `ReflectionEffect`.
- Implement `save_obj` for `Scene`.

### Fixed

- Using `i32` instead of `c_uint` in some places would fail to compile on some platforms.

## [0.3.0] - 2025-03-23

### Added

- Implement `Send` trait for `Scene`.
- Add assertion to enforce the number of channels in the input buffer when applying the reflection effect or the ambisonics encode effect.

### Changed

- Refactor `SimulationInputs`, `SimulationSettings`.
- Change signatures of `Scene::add_static_mesh`, `Scene::remove_static_mesh`, `Scene::add_instanced_mesh`, `Scene::remove_instanced_mesh`, `Scene::update_instanced_mesh_transform`, `Scene::commit` to take `&mut self` instead of `&self`.
- Remove fields that can be inferred from `StaticMeshSettings`, borrow slices instead of owning data.

### Fixed

- `SimulationOutputs` would remain zeroed.
- `DirectEffectParams` fields would always be `None` since `IPLDirectEffectParams.flags` is not set when retrieving simulation results for a source.

## [0.2.0] - 2025-03-17

### Changed

- Made inner fields of `OpenClDeviceList`, `AmbisonicsBinauralEffect`, `AmbisonicsDecodeEffect`, `AmbisonicsEncodeEffect`, `AmbisonicsPanningEffect`, `AmbisonicsRotationEffect`, `DirectEffect` private.

### Fixed

- Order assertions of ambisonics effects.

## [0.1.0] - 2025-03-08

### Added

- Initial version supporting Steam Audio 4.6.0.
- First public release.
