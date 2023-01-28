use std::{fmt::Debug, path::PathBuf, time::SystemTime};

use color_eyre::Result;
use serde::Deserialize;

use crate::{
    image::Image,
    messages::{IncomingMessage, OutgoingMessage},
    samples::Samples,
};

use super::{CameraPosition, Joints, Leds, SensorData};

pub trait Interface {
    fn read_from_microphones(&self) -> Result<Samples>;

    fn get_now(&self) -> SystemTime;
    fn get_ids(&self) -> Ids;
    fn get_paths(&self) -> Paths;
    fn read_from_sensors(&self) -> Result<SensorData>;
    fn write_to_actuators(&self, positions: Joints, stiffnesses: Joints, leds: Leds) -> Result<()>;

    fn read_from_network(&self) -> Result<IncomingMessage>;
    fn write_to_network(&self, message: OutgoingMessage) -> Result<()>;

    fn read_from_camera(&self, camera_position: CameraPosition) -> Result<Image>;
}

#[derive(Clone, Debug)]
pub struct Ids {
    pub body_id: String,
    pub head_id: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Paths {
    pub parameters: PathBuf,
    pub motions: PathBuf,
    pub neural_networks: PathBuf,
}
