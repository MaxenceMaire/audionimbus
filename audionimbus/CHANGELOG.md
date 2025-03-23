# Changelog

## [Unreleased]

### Added

- Implement `Send` trait for `Scene`.
- Add assertion to enforce the number of channels in the input buffer when applying the reflection effect.

### Changed

- Refactor `SimulationInputs`, `SimulationSettings`.
- Change signatures of `Scene::add_static_mesh`, `Scene::remove_static_mesh`, `Scene::add_instanced_mesh`, `Scene::remove_instanced_mesh`, `Scene::update_instanced_mesh_transform`, `Scene::commit` to take `&mut self` instead of `&self`.

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
