# Changelog

## [Unreleased]

### Added

- Implement `mix`, `downmix`, `convert_ambisonics`, `convert_ambisonics_into` methods for `AudioBuffer`.
- Implement `tail`, `tail_size` methods for `PanningEffect`, `PathEffect`, `AmbisonicsBinauralEffect`, `AmbisonicsDecodeEffect`, `AmbisonicsDecodeEffect`, `AmbisonicsPanningEffect`, `AmbisonicsRotationEffect`, `BinauralEffect`, `DirectEffect`, `DirectEffect`, `PathEffect`, `ReflectionEffect`, `VirtualSurroundEffect`.
- Implement `tail_into_mixer` for `ReflectionEffect`.

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
