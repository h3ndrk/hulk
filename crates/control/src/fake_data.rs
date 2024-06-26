use std::{net::SocketAddr, time::Duration};

use color_eyre::Result;
use linear_algebra::Isometry2;
use serde::{Deserialize, Serialize};

use context_attribute::context;
use coordinate_systems::{Field, Ground};
use framework::MainOutput;
use spl_network_messages::HulkMessage;
use types::{
    ball_position::{BallPosition, HypotheticalBallPosition},
    cycle_time::CycleTime,
    fall_state::FallState,
    filtered_whistle::FilteredWhistle,
    game_controller_state::GameControllerState,
    joints::head::HeadJoints,
    obstacles::Obstacle,
    parameters::{BallFilterParameters, CameraMatrixParameters},
    penalty_shot_direction::PenaltyShotDirection,
    primary_state::PrimaryState,
    sensor_data::SensorData,
};

#[derive(Deserialize, Serialize)]
pub struct FakeData {}

#[context]
#[allow(dead_code)]
pub struct CreationContext {
    maximum_velocity: Parameter<HeadJoints<f32>, "head_motion.maximum_velocity">,
    top_camera_matrix_parameters:
        Parameter<CameraMatrixParameters, "camera_matrix_parameters.vision_top">,
    ball_filter: Parameter<BallFilterParameters, "ball_filter">,
}

#[context]
#[allow(dead_code)]
pub struct CycleContext {
    glance_angle: Parameter<f32, "look_at.glance_angle">,
}

#[context]
#[derive(Default)]
pub struct MainOutputs {
    pub ball_position: MainOutput<Option<BallPosition<Ground>>>,
    pub cycle_time: MainOutput<CycleTime>,
    pub fall_state: MainOutput<FallState>,
    pub filtered_whistle: MainOutput<FilteredWhistle>,
    pub game_controller_state: MainOutput<Option<GameControllerState>>,
    pub game_controller_address: MainOutput<Option<SocketAddr>>,
    pub has_ground_contact: MainOutput<bool>,
    pub hulk_messages: MainOutput<Vec<HulkMessage>>,
    pub hypothetical_ball_positions: MainOutput<Vec<HypotheticalBallPosition<Ground>>>,
    pub obstacles: MainOutput<Vec<Obstacle>>,
    pub penalty_shot_direction: MainOutput<Option<PenaltyShotDirection>>,
    pub primary_state: MainOutput<PrimaryState>,
    pub ground_to_field: MainOutput<Option<Isometry2<Ground, Field>>>,
    pub sensor_data: MainOutput<SensorData>,
    pub stand_up_front_estimated_remaining_duration: MainOutput<Option<Duration>>,
    pub stand_up_back_estimated_remaining_duration: MainOutput<Option<Duration>>,
}

impl FakeData {
    pub fn new(_context: CreationContext) -> Result<Self> {
        Ok(Self {})
    }

    pub fn cycle(&mut self, _context: CycleContext) -> Result<MainOutputs> {
        Ok(MainOutputs::default())
    }
}
