use context_attribute::context;
use framework::{
    AdditionalOutput, HistoricInput, MainOutput, Input, Parameter, PerceptionInput,
};
use nalgebra::{Isometry2, Point2};
use types::{
    DetectedRobots, FieldDimensions, Obstacle, ObstacleFilterHypothesis, SensorData, SonarObstacle,
};

pub struct ObstacleFilter {}

#[context]
pub struct NewContext {
    pub field_dimensions: Parameter<FieldDimensions, "field_dimensions">,
    pub goal_post_obstacle_radius:
        Parameter<f32, "control/obstacle_filter/goal_post_obstacle_radius">,
    // pub obstacle_filter:
    //     Parameter<ObstacleFilter, "control/obstacle_filter">,
    pub robot_obstacle_radius_at_foot_height:
        Parameter<f32, "control/obstacle_filter/robot_obstacle_radius_at_foot_height">,
    pub robot_obstacle_radius_at_hip_height:
        Parameter<f32, "control/obstacle_filter/robot_obstacle_radius_at_hip_height">,
    pub unknown_obstacle_radius: Parameter<f32, "control/obstacle_filter/unknown_obstacle_radius">,
}

#[context]
pub struct CycleContext {
    pub obstacle_filter_hypotheses:
        AdditionalOutput<Vec<ObstacleFilterHypothesis>, "obstacle_filter_hypotheses">,

    pub current_odometry_to_last_odometry:
        HistoricInput<Isometry2<f32>, "current_odometry_to_last_odometry">,
    pub network_robot_obstacles: HistoricInput<Vec<Point2<f32>>, "network_robot_obstacles">,
    pub robot_to_field: HistoricInput<Isometry2<f32>, "robot_to_field">,
    pub sonar_obstacle: HistoricInput<SonarObstacle, "sonar_obstacle">,

    pub sensor_data: Input<SensorData, "sensor_data?">,

    pub field_dimensions: Parameter<FieldDimensions, "field_dimensions">,
    pub goal_post_obstacle_radius:
        Parameter<f32, "control/obstacle_filter/goal_post_obstacle_radius">,
    // pub obstacle_filter:
    //     Parameter<ObstacleFilter, "control/obstacle_filter">,
    pub robot_obstacle_radius_at_foot_height:
        Parameter<f32, "control/obstacle_filter/robot_obstacle_radius_at_foot_height">,
    pub robot_obstacle_radius_at_hip_height:
        Parameter<f32, "control/obstacle_filter/robot_obstacle_radius_at_hip_height">,
    pub unknown_obstacle_radius: Parameter<f32, "control/obstacle_filter/unknown_obstacle_radius">,

    pub detected_robots_bottom: PerceptionInput<DetectedRobots, "VisionBottom", "detected_robots">,
    pub detected_robots_top: PerceptionInput<DetectedRobots, "VisionTop", "detected_robots">,
}

#[context]
#[derive(Default)]
pub struct MainOutputs {
    pub obstacles: MainOutput<Vec<Obstacle>>,
}

impl ObstacleFilter {
    pub fn new(_context: NewContext) -> anyhow::Result<Self> {
        Ok(Self {})
    }

    pub fn cycle(&mut self, _context: CycleContext) -> anyhow::Result<MainOutputs> {
        Ok(MainOutputs::default())
    }
}
