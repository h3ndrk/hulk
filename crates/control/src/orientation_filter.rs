use color_eyre::Result;
use context_attribute::context;
use framework::MainOutput;
use nalgebra::UnitComplex;
use types::{
    configuration::OrientationFilter as OrientationFilterConfiguration, SensorData, SolePressure,
    SupportFoot,
};

pub struct OrientationFilter {}

#[context]
pub struct CreationContext {
    pub orientation_filter: Parameter<OrientationFilterConfiguration, "control.orientation_filter">,
}

#[context]
pub struct CycleContext {
    pub sensor_data: Input<SensorData, "sensor_data">,
    pub sole_pressure: Input<SolePressure, "sole_pressure">,
    pub support_foot: RequiredInput<Option<SupportFoot>, "support_foot?">,

    pub orientation_filter: Parameter<OrientationFilterConfiguration, "control.orientation_filter">,
}

#[context]
#[derive(Default)]
pub struct MainOutputs {
    pub robot_orientation: MainOutput<Option<UnitComplex<f32>>>,
}

impl OrientationFilter {
    pub fn new(_context: CreationContext) -> Result<Self> {
        Ok(Self {})
    }

    pub fn cycle(&mut self, _context: CycleContext) -> Result<MainOutputs> {
        Ok(MainOutputs::default())
    }
}
