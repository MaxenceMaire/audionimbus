# Changelog

## [Unreleased]

### Fixed

- Fixed HRTF loading from SOFA files where the filename string was being dropped before the FFI call completed, causing load failures.
- Fixed a segmentation fault caused by static and instanced meshes not living long enough when added to scenes.
- Fixed a segmentation fault in bake_path when no progress callback was provided, caused by an upstream Steam Audio bug; a no-op callback is now used as a workaround.
- Fixed a segmentation fault caused by running a pathing simulation without probes.
- Fixed a segmentation fault caused by running a reflections simulation without having set a scene.
- Fixed a segmentation fault when applying a direct effect on audio buffers that have a number of channels different from that specified when creating the effect.
- Fixed a segmentation fault when calling `DirectEffect::tail` with an output buffer that has a number of channels different from that specified when creating the effect.
- Fixed a segmentation fault when applying a pathing effect on an input buffer other than mono or passing an output buffer that has a number of channels different from that needed for the ambisonics order specified when creating the effect.
- Fixed a segmentation fault when calling `PathEffect::tail` with an output buffer that has a number of channels other than that needed for the ambisonics order specified when creating the effect.
- Fixed a segmentation fault when applying a reflection effect on an input buffer other than mono or with an output buffer that has a number of channels other than that of the impulse response specified when creating the effect.
- Fixed a segmentation fault when calling `ReflectionEffect::tail` or `ReflectionEffect::tail_into_mixer` with an output buffer that has a number of channels other than that of the impulse response specified when creating the effect.
- Fixed a segmentation fault where `ReflectionEffect::apply_into_mixer` was missing the output buffer.
- Fixed a segmentation fault when applying a binaural effect on an input buffer other than mono or stereo, or passing an output buffer that has more than two channels.
- Fixed a segmentation fault when calling `BinauralEffect::tail` with an output buffer that has more than two channels.
- Fixed a segmentation fault when applying a panning effect on an input buffer other than mono, or passing an output buffer that has a number of channels different from that needed for the speaker layout specified when creating the effect.
- Fixed a segmentation fault when calling `BinauralEffect::tail` with an output buffer that has a number of channels different from that needed for the speaker layout specified when creating the effect.
- Fixed a segmentation fault when applying a virtual surround effect on an input buffer that has a number of channels different from that needed for the speaker layout specified when creating the effect, or an output buffer that does not have two channels.
- Fixed a segmentation fault when calling `BinauralEffect::tail` with an output buffer that does not have two channels.
- Fixed a segmentation fault when applying an ambisonics encode effect on an input buffer does not have exactly one channel or an output buffer that does not have the correct number of channels for the ambisonics order.
- Fixed a segmentation fault when calling `AmbisonicsEncodeEffect::tail` with an output buffer that does not have the correct number of channels for the ambisonics order.

### Changed

- Mark `air_absorption` function as `unsafe` since it calls `iplAirAbsorptionCalculate` which causes a segfault when using a callback.
- Improved documentation and added examples.
- Make `NUM_BANDS` constant private.
- Rename the `null` methods of `EmbreeDevice`, `OpenClDevice`, `RadeonRaysDevice` and `TrueAudioNextDevice` into `try_new` for consistency.
- `InstancedMeshSettings` now takes a reference to the sub-scene.
- `Scene::add_static_mesh` and `Scene::add_instanced_mesh` now return handles.
- `Scene::remove_static_mesh` and `Scene::remove_instanced_mesh` now take handles as arguments instead of references to `StaticMesh` and `InstancedMesh`.
- `Scene::add_probe` now takes the probe by value instead of by reference.
- `Simulator::run_pathing` now returns an error if the simulator contains no probes.
- `ProbeBatch::commit` now takes `&mut self` instead of `&self`.
- `Simulator::run_reflections` now returns an error if a scene has not been set and committed to the simulator.
- `DirectEffect::apply` now returns an `EffectError` when the input of output buffers have a number of channels different from that specified when creating the effect.
- `PathEffect::apply` now returns an `EffectError` when the input buffer is not mono or the output buffer has a number of channels different from that needed for the ambisonics order specified when creating the effect.
- `PathEffect::tail` now returns an `EffectError` when the output buffer has a number of channels different from that needed for the ambisonics order specified when creating the effect.
- `ReflectionEffect::apply_into_mixer` takes an additional `output_buffer` argument.
- `ReflectionEffect::apply` and `ReflectionEffect::apply_into_mixer` now return an `EffectError` when the input buffer is not mono or the output buffer has a number of channels other than that of the impulse response specified when creating the effect.
- `ReflectionEffect::tail` and `ReflectionEffect::tail_into_mixer` now return an `EffectError` when the output buffer has a number of channels other than that of the impulse response specified when creating the effect.
- `BinauralEffect::apply` now returns an `EffectError` when the input buffer is not mono or stereo, or the output buffer has more than two channels.
- `BinauralEffect::tail` now returns an `EffectError` when the output buffer has more than two channels.
- `PanningEffect::apply` now returns an `EffectError` when the input buffer is not mono, or the output buffer has a number of channels different from that needed for the speaker layout specified when creating the effect.
- `PanningEffect::tail` now returns an `EffectError` when the output buffer has a number of channels different from that needed for the speaker layout specified when creating the effect.
- `VirtualSurroundEfect::apply` now returns an `EffectError` when the input buffer that has a number of channels different from that needed for the speaker layout specified when creating the effect, or the output buffer that does not have two channels.
- `VirtualSurroundEffect::tail` now returns an `EffectError` when the output buffer does not have two channels.
- `AmbisonicsEncodeEfect::apply` now returns an `EffectError` when the input buffer does not have exactly one channel or the output buffer that does not have the correct number of channels for the ambisonics order.
- `AmbisonicsEncodeEffect::tail` now returns an `EffectError` when the output buffer that does not have the correct number of channels for the ambisonics order.

### Added

- Implement `Default` trait for `Context` using default `ContextSettings`.
- Derive `PartialEq` trait for `Matrix`, `Triangle`, `Sphere`.
- Increase the test coverage.
- Add `num_ambisonics_channels` const function to compute the number of channels reuqired given an ambisonics order.
- Add `Matrix3` and `Matrix4` type aliases.
- Add `StaticMeshHandle` and `InstancedMeshHandle` structs (returned by `Scene::add_static_mesh` and `Scene::add_instanced_mesh` respectively).
- Add `slotmap` lib dependency.
- Add method `ProbeBatch::committed_num_probes`.
- `SpeakerLayout` now implements `Clone` and `Display`.
- Add `EffectError` for errors that can occur when applying audio effects.
- `SteamAudioError` implements `PartialEq`, `Copy` and `Clone`.
- Add `ChannelRequirement` to specify the channel count requirement for an audio buffer.

### Removed

- Removed the `From<&HrtfSettings>` trait implementation for `audionimbus_sys::IPLHRTFSettings` in favor of the new `to_ffi` method on `HrtfSettings`, which allows the optional filename variable to be kept alive for FFI calls.
- Make the `null` methods of `EmbreeDevice`, `OpenClDevice`, `RadeonRaysDevice` and `TrueAudioNextDevice` only public to the crate.

## [0.11.0] - 2026-01-14

### Changed

- Upgrade to Steam Audio v4.8.0.

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
