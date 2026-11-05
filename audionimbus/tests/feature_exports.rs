#[cfg(feature = "dynamic")]
use audionimbus::dynamic;
use audionimbus::{Context, Scene, geometry, prelude};

#[cfg(any(feature = "static", not(feature = "dynamic")))]
#[test]
fn exports_static_scene_paths() {
    let scene: geometry::Scene = Scene::try_new(&Context::default()).unwrap();
    #[cfg(feature = "static")]
    let scene: audionimbus::r#static::Scene = scene;
    let _scene: prelude::Scene = scene;
}

#[cfg(all(feature = "dynamic", not(feature = "static")))]
#[test]
fn exports_runtime_scene_paths() {
    let scene: dynamic::Scene =
        Scene::try_new(&Context::default(), audionimbus::RayTracerSettings::Default).unwrap();
    let scene: geometry::Scene = scene;
    let _scene: prelude::Scene = scene;
}

#[cfg(any(feature = "static", feature = "dynamic"))]
#[test]
fn explicit_paths_export_scenes() {
    let context = Context::default();
    #[cfg(feature = "static")]
    audionimbus::r#static::Scene::try_new(&context).unwrap();

    #[cfg(feature = "dynamic")]
    dynamic::Scene::try_new(&context, dynamic::RayTracerSettings::Default).unwrap();
}
