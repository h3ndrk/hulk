use context_attribute::context;
use framework::{AdditionalOutput, MainOutput, OptionalInput, Parameter};
use types::{
    configuration::BallDetection as BallDetectionConfiguration, Ball, CameraMatrix,
    CandidateEvaluation, PerspectiveGridCandidates,
};

pub struct BallDetection {}

#[context]
pub struct NewContext {
    pub ball_detection: Parameter<BallDetectionConfiguration, "$cycler_instance/ball_detection">,
    pub ball_radius: Parameter<f32, "field_dimensions/ball_radius">,
}

#[context]
pub struct CycleContext {
    pub ball_candidates: AdditionalOutput<Vec<CandidateEvaluation>, "ball_candidates">,

    pub camera_matrix: OptionalInput<CameraMatrix, "camera_matrix?">,
    pub perspective_grid_candidates:
        OptionalInput<PerspectiveGridCandidates, "perspective_grid_candidates?">,

    pub ball_detection: Parameter<BallDetectionConfiguration, "$cycler_instance/ball_detection">,
    pub ball_radius: Parameter<f32, "field_dimensions/ball_radius">,
}

#[context]
#[derive(Default)]
pub struct MainOutputs {
    pub balls: MainOutput<Vec<Ball>>,
}

impl BallDetection {
    pub fn new(_context: NewContext) -> anyhow::Result<Self> {
        Ok(Self {})
    }

    pub fn cycle(&mut self, _context: CycleContext) -> anyhow::Result<MainOutputs> {
        Ok(MainOutputs::default())
    }
}
