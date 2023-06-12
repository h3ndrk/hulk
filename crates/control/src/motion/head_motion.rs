use std::f32::consts::PI;

use color_eyre::Result;
use context_attribute::context;
use framework::MainOutput;
use types::{
    CycleTime, HeadJoints, HeadJointsCommand, HeadMotion as HeadMotionCommand, MotionCommand,
    SensorData,
};

#[derive(Default)]
pub struct HeadMotion {}

#[context]
pub struct CreationContext {
    pub center_head_position: Parameter<HeadJoints<f32>, "center_head_position">,
    pub inner_maximum_pitch: Parameter<f32, "head_motion.inner_maximum_pitch">,
    pub maximum_velocity: Parameter<HeadJoints<f32>, "head_motion.maximum_velocity">,
    pub outer_maximum_pitch: Parameter<f32, "head_motion.outer_maximum_pitch">,
    pub outer_yaw: Parameter<f32, "head_motion.outer_yaw">,
}

#[context]
pub struct CycleContext {
    pub center_head_position: Parameter<HeadJoints<f32>, "center_head_position">,
    pub inner_maximum_pitch: Parameter<f32, "head_motion.inner_maximum_pitch">,
    pub maximum_velocity: Parameter<HeadJoints<f32>, "head_motion.maximum_velocity">,
    pub outer_maximum_pitch: Parameter<f32, "head_motion.outer_maximum_pitch">,
    pub outer_yaw: Parameter<f32, "head_motion.outer_yaw">,

    pub look_around: Input<HeadJoints<f32>, "look_around">,
    pub look_at: Input<HeadJoints<f32>, "look_at">,
    pub motion_command: Input<MotionCommand, "motion_command">,
    pub sensor_data: Input<SensorData, "sensor_data">,
    pub cycle_time: Input<CycleTime, "cycle_time">,
    pub has_ground_contact: Input<bool, "has_ground_contact">,
}

#[context]
#[derive(Default)]
pub struct MainOutputs {
    pub head_joints_command: MainOutput<HeadJointsCommand<f32>>,
}

impl HeadMotion {
    pub fn new(_context: CreationContext) -> Result<Self> {
        Ok(Self {})
    }

    pub fn cycle(&mut self, context: CycleContext) -> Result<MainOutputs> {
        let HeadJointsCommand {
            positions: raw_positions,
            stiffnesses,
        } = context
            .has_ground_contact
            .then(|| Self::joints_from_motion(&context))
            .unwrap_or_else(|| HeadJointsCommand {
                positions: Default::default(),
                stiffnesses: HeadJoints::fill(0.8),
            });

        let maximum_movement =
            *context.maximum_velocity * context.cycle_time.last_cycle_duration.as_secs_f32();

        let controlled_positions = HeadJoints {
            yaw: context.sensor_data.positions.head.yaw
                + (raw_positions.yaw - context.sensor_data.positions.head.yaw)
                    .clamp(-maximum_movement.yaw, maximum_movement.yaw),
            pitch: context.sensor_data.positions.head.pitch
                + (raw_positions.pitch - context.sensor_data.positions.head.pitch)
                    .clamp(-maximum_movement.pitch, maximum_movement.pitch),
        };

        let maximum_pitch = if controlled_positions.yaw.abs() >= *context.outer_yaw {
            *context.outer_maximum_pitch
        } else {
            let interpolation_factor =
                0.5 * (1.0 + (PI / *context.outer_yaw * controlled_positions.yaw).cos());
            *context.outer_maximum_pitch
                + interpolation_factor
                    * (*context.inner_maximum_pitch - *context.outer_maximum_pitch)
        };

        let clamped_pitch = controlled_positions.pitch.clamp(0.0, maximum_pitch);
        let clamped_positions = HeadJoints {
            pitch: clamped_pitch,
            yaw: controlled_positions.yaw,
        };

        Ok(MainOutputs {
            head_joints_command: HeadJointsCommand {
                positions: clamped_positions,
                stiffnesses,
            }
            .into(),
        })
    }

    pub fn joints_from_motion(context: &CycleContext) -> HeadJointsCommand<f32> {
        let stiffnesses = HeadJoints::fill(0.8);
        match context.motion_command.head_motion() {
            Some(HeadMotionCommand::Center) => HeadJointsCommand {
                positions: *context.center_head_position,
                stiffnesses,
            },
            Some(HeadMotionCommand::LookAround | HeadMotionCommand::SearchForLostBall) => {
                HeadJointsCommand {
                    positions: *context.look_around,
                    stiffnesses,
                }
            }
            Some(HeadMotionCommand::LookAt { .. })
            | Some(HeadMotionCommand::LookLeftAndRightOf { .. }) => HeadJointsCommand {
                positions: *context.look_at,
                stiffnesses,
            },
            Some(HeadMotionCommand::Unstiff) => HeadJointsCommand {
                positions: context.sensor_data.positions.head,
                stiffnesses: HeadJoints::fill(0.0),
            },
            Some(HeadMotionCommand::ZeroAngles) | None => HeadJointsCommand {
                positions: Default::default(),
                stiffnesses,
            },
        }
    }
}
