use audionimbus::*;

#[test]
fn test_directivity_attenuation() {
    let context = Context::default();

    let source = CoordinateSystem::default();
    let listener = Point::new(0.0, 0.0, 0.0);

    let directivity = Directivity::default();

    let directivity_attenuation = directivity_attenuation(&context, source, listener, &directivity);

    assert_eq!(directivity_attenuation, 0.70710677);
}
