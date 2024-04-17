#![recursion_limit = "256"]
use std::fs::create_dir_all;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env::args, path::PathBuf, sync::Arc};

use color_eyre::{
    eyre::{Result, WrapErr},
    install,
};
use hardware::{
    ActuatorInterface, CameraInterface, NetworkInterface, PathsInterface, RecordingInterface,
    SpeakerInterface, TimeInterface,
};
use types::{camera_position::CameraPosition, hardware::Paths, ycbcr422_image::YCbCr422Image};

use crate::execution::Replayer;

pub trait HardwareInterface:
    ActuatorInterface
    + CameraInterface
    + NetworkInterface
    + PathsInterface
    + RecordingInterface
    + SpeakerInterface
    + TimeInterface
{
}

include!(concat!(env!("OUT_DIR"), "/generated_code.rs"));

struct ImageExtractorHardwareInterface;

impl ActuatorInterface for ImageExtractorHardwareInterface {
    fn write_to_actuators(
        &self,
        _positions: types::joints::Joints<f32>,
        _stiffnesses: types::joints::Joints<f32>,
        _leds: types::led::Leds,
    ) -> Result<()> {
        Ok(())
    }
}

impl CameraInterface for ImageExtractorHardwareInterface {
    fn read_from_camera(&self, _camera_position: CameraPosition) -> Result<YCbCr422Image> {
        panic!("Replayer cannot produce data from hardware")
    }
}

impl NetworkInterface for ImageExtractorHardwareInterface {
    fn read_from_network(&self) -> Result<types::messages::IncomingMessage> {
        panic!("asd")
    }

    fn write_to_network(&self, _message: types::messages::OutgoingMessage) -> Result<()> {
        Ok(())
    }
}

impl PathsInterface for ImageExtractorHardwareInterface {
    fn get_paths(&self) -> Paths {
        Paths {
            motions: "etc/motions".into(),
            neural_networks: "etc/neural_networks".into(),
            sounds: "etc/sounds".into(),
        }
    }
}

impl RecordingInterface for ImageExtractorHardwareInterface {
    fn should_record(&self) -> bool {
        panic!("sdasd")
    }

    fn set_whether_to_record(&self, _enable: bool) {}
}

impl SpeakerInterface for ImageExtractorHardwareInterface {
    fn write_to_speakers(&self, _request: types::audio::SpeakerRequest) {}
}

impl TimeInterface for ImageExtractorHardwareInterface {
    fn get_now(&self) -> SystemTime {
        SystemTime::now()
    }
}

impl HardwareInterface for ImageExtractorHardwareInterface {}

#[derive(Debug, serde::Serialize)]
struct Metadata {
    camera_matrix: Option<projection::camera_matrix::CameraMatrix>,
    robot_kinematics: types::robot_kinematics::RobotKinematics,
    ground_to_field_of_home_after_coin_toss_before_second_half:
        Option<linear_algebra::Isometry2<coordinate_systems::Ground, coordinate_systems::Field>>,
}

fn main() -> Result<()> {
    install()?;

    let replay_path = args()
        .nth(1)
        .expect("expected replay path as first parameter");

    let output_folder = PathBuf::from(
        args()
            .nth(2)
            .expect("expected output path as second parameter"),
    );

    let parameters_directory = args().nth(3).unwrap_or(replay_path.clone());
    let id = "replayer".to_string();

    let mut replayer = Replayer::new(
        Arc::new(ImageExtractorHardwareInterface),
        parameters_directory,
        id.clone(),
        id,
        replay_path,
    )
    .wrap_err("failed to create image extractor")?;

    let control_reader = replayer.control_reader();
    let vision_top_reader = replayer.vision_top_reader();
    let vision_bottom_reader = replayer.vision_bottom_reader();

    for (instance_name, reader) in [
        ("VisionTop", vision_top_reader),
        ("VisionBottom", vision_bottom_reader),
    ] {
        let output_folder = &output_folder.join(instance_name);
        create_dir_all(output_folder).expect("failed to create output folder");

        let unknown_indices_error_message =
            format!("could not find recording indices for `{instance_name}`");
        let timings: Vec<_> = replayer
            .get_recording_indices()
            .get(instance_name)
            .expect(&unknown_indices_error_message)
            .iter()
            .collect();

        for timing in timings {
            let frame = replayer
                .get_recording_indices_mut()
                .get_mut(instance_name)
                .map(|index| {
                    index
                        .find_latest_frame_up_to(timing.timestamp)
                        .expect("failed to find latest frame")
                })
                .expect(&unknown_indices_error_message);

            if let Some(frame) = frame {
                replayer
                    .replay(instance_name, frame.timing.timestamp, &frame.data)
                    .expect("failed to replay frame");

                let vision_database = reader.next();
                let control_database = control_reader.next();
                let output_file = output_folder.join(format!(
                    "{}.png",
                    frame
                        .timing
                        .timestamp
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                ));
                serde_json::to_writer_pretty(
                    std::fs::File::create(output_file.with_extension("json")).unwrap(),
                    &Metadata {
                        camera_matrix: vision_database.main_outputs.camera_matrix.clone(),
                        robot_kinematics: control_database.main_outputs.robot_kinematics.clone(),
                        ground_to_field_of_home_after_coin_toss_before_second_half:
                            control_database
                                .main_outputs
                                .ground_to_field_of_home_after_coin_toss_before_second_half
                                .clone(),
                    },
                )
                .unwrap();
                vision_database
                    .main_outputs
                    .image
                    .save_to_ycbcr_444_file(output_file)
                    .expect("failed to write file");
            }
        }
    }

    Ok(())
}
