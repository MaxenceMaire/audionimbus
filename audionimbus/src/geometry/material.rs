/// The acoustic properties of a surface.
///
/// You can specify the acoustic material properties of each triangle, although typically many triangles will share a common material.
///
/// The acoustic material properties are specified for three frequency bands with center frequencies of 400 Hz, 2.5 KHz, and 15 KHz.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Material {
    /// Fraction of sound energy absorbed at low, middle, high frequencies.
    ///
    /// Between 0.0 and 1.0.
    pub absorption: [f32; 3],

    /// Fraction of sound energy scattered in a random direction on reflection.
    ///
    /// Between 0.0 (pure specular) and 1.0 (pure diffuse).
    pub scattering: f32,

    /// Fraction of sound energy transmitted through at low, middle, high frequencies.
    ///
    /// Between 0.0 and 1.0. Only used for direct occlusion calculations.
    pub transmission: [f32; 3],
}

impl Material {
    pub const GENERIC: Self = Self {
        absorption: [0.10, 0.20, 0.30],
        scattering: 0.05,
        transmission: [0.100, 0.050, 0.030],
    };

    pub const BRICK: Self = Self {
        absorption: [0.03, 0.04, 0.07],
        scattering: 0.05,
        transmission: [0.015, 0.015, 0.015],
    };

    pub const CONCRETE: Self = Self {
        absorption: [0.05, 0.07, 0.08],
        scattering: 0.05,
        transmission: [0.015, 0.002, 0.001],
    };

    pub const CERAMIC: Self = Self {
        absorption: [0.01, 0.02, 0.02],
        scattering: 0.05,
        transmission: [0.060, 0.044, 0.011],
    };

    pub const GRAVEL: Self = Self {
        absorption: [0.60, 0.70, 0.80],
        scattering: 0.05,
        transmission: [0.031, 0.012, 0.008],
    };

    pub const CARPET: Self = Self {
        absorption: [0.24, 0.69, 0.73],
        scattering: 0.05,
        transmission: [0.020, 0.005, 0.003],
    };

    pub const GLASS: Self = Self {
        absorption: [0.06, 0.03, 0.02],
        scattering: 0.05,
        transmission: [0.060, 0.044, 0.011],
    };

    pub const PLASTER: Self = Self {
        absorption: [0.12, 0.06, 0.04],
        scattering: 0.05,
        transmission: [0.056, 0.056, 0.004],
    };

    pub const WOOD: Self = Self {
        absorption: [0.11, 0.07, 0.06],
        scattering: 0.05,
        transmission: [0.070, 0.014, 0.005],
    };

    pub const METAL: Self = Self {
        absorption: [0.20, 0.07, 0.06],
        scattering: 0.05,
        transmission: [0.200, 0.025, 0.010],
    };

    pub const ROCK: Self = Self {
        absorption: [0.13, 0.20, 0.24],
        scattering: 0.05,
        transmission: [0.015, 0.002, 0.001],
    };
}

impl From<Material> for audionimbus_sys::IPLMaterial {
    fn from(material: Material) -> Self {
        audionimbus_sys::IPLMaterial {
            absorption: material.absorption,
            scattering: material.scattering,
            transmission: material.transmission,
        }
    }
}
