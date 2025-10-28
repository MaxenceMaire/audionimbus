# Changelog

## [0.10.0] - 2025-10-28

### Fixed

- Disabling reflections would cause the ambisonic order to default to 0, leading to out-of-bounds writes and memory corruption.

## [0.9.1] - 2025-10-19

### Changed

- `Simulator::remove_source` now takes `&self` instead of `&mut self`.
- `directivity_attenuation` now takes `source` and `listener` by value instead of by reference.
- `air_absorption` now returns `Equalizer<3>` instead of `[f32; 3]`.

## [0.9.0] - 2025-10-13

### Added

- Support for borrowed channel pointers in `AudioBuffer`, in addition to the existing owned version.
- Support for different audio buffer lifetimes for `mix`, `downmix`, `convert_ambisonics_into`.
- Add `Default` trait to `DistanceAttenuationModel`, `DeviationModel`, `AirAbsorptionModel`, `Sphere`, `SceneSettings`, `Matrix<f32, 3, 3>`, `Matrix<f32, 4, 4>`, `Material`, `SimdLevel`.
- Add `PartialEq` trait to `Vector3`, `Material`, `CoordinateSystem`, `ReflectionEffectIR`, `TrueAudioNextDevice`, `Hrtf`.
- Implement `Send` for `ReflectionEffectIR`, `ReflectionEffectParams`.
- Add `try_from_slices` method to construct an `AudioBuffer` from `&[&[f32]]` data.
- Add a `channels_mut` method for iterating over mutable channels.
- Add `ShCoeffs` for spherical harmonic coefficients.

### Fixed

- Changed the signature of `AudioBuffer::downmix` to match `AudioBuffer::mix`.

### Changed

- Use `u32` instead of `usize` for frame size, sampling rate, number of channels, ambisonics order, and other similar types of values.
- The git submodule for Steam Audio now uses HTTPS to avoid authentication.
- Update `bitflages` dependency to v2.9.
- Relaxed trait constraints for the `try_new` method of `AudioBuffer`.
- `Simulator::add_source` now takes a simple reference to `self` instead of a mutable reference.

### Removed

- `try_new_borrowed` method on `AudioBuffer`.

## [0.8.3] - 2025-10-04

### Fixed

- Removed the `doc_auto_cfg` feature as it has been stabilized and it would cause the doc build to fail.

## [0.8.2] - 2025-10-04

### Fixed

- The documentation would incorrectly instruct users to install Steam Audio v4.6.1 instead of v4.7.0.
- Fixed the casing of the `user_data` argument of `PathingVisualizationCallback`.

## [0.8.1] - 2025-08-25

### Changed

- The `unzip` command is no longer a requirement for using the `auto-install` feature.

### Fixed

- The `auto-install` feature would fail on Windows because of the missing `phonon.lib` linkage.

## [0.8.0] - 2025-08-19

### Added

- Support for Steam Audio v.4.7.0 via `audionimbus-sys` v.4.7.0.
- Support for custom pathing deviation model.
- `EnergyField`, `Reconstructor`, `ImpulseResponse` and related methods.
- Methods to list OpenCL devices.
- Baking cancellation for pathing and reflections.
- Missing documentation on simulation.
- `auto-install` feature to automatically download and install the Steam Audio library.

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
