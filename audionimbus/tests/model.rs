use audionimbus::*;

#[test]
fn test_distance_attenuation() {
    let context = Context::default();

    let source = Point::new(1.0, 1.0, 1.0);
    let listener = Point::new(0.0, 0.0, 0.0);

    let distance_attenuation_model = DistanceAttenuationModel::default();

    let distance_attenuation =
        distance_attenuation(&context, source, listener, &distance_attenuation_model);

    assert_eq!(distance_attenuation, 0.57735026);
}

#[test]
fn test_air_absorption() {
    let context = Context::default();

    let source = Point::new(1.0, 1.0, 1.0);
    let listener = Point::new(0.0, 0.0, 0.0);

    let air_absorption_model = AirAbsorptionModel::default();

    let air_absorption = air_absorption(&context, &source, &listener, &air_absorption_model);

    assert_eq!(air_absorption.0, [0.99965364, 0.9970598, 0.96896833]);
}

#[test]
fn test_directivity_attenuation() {
    let context = Context::default();

    let source = CoordinateSystem::default();
    let listener = Point::new(0.0, 0.0, 0.0);

    let directivity = Directivity::default();

    let directivity_attenuation = directivity_attenuation(&context, source, listener, &directivity);

    assert_eq!(directivity_attenuation, 0.70710677);
}
