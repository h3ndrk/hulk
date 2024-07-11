use std::fs::{read_dir, File};

use coordinate_systems::{Field, Ground};
use framework::AdditionalOutput;
use image::RgbImage;
use linear_algebra::Isometry2;
use projection::camera_matrix::CameraMatrix;
use serde::Deserialize;
use serde_json::from_reader;
use types::{
    color::{self, Intensity},
    field_color::{FieldColor, FieldColorFunction},
    interpolated::Interpolated,
    parameters::{EdgeDetectionSourceParameters, MedianModeParameters},
    robot_kinematics::RobotKinematics,
    ycbcr422_image::YCbCr422Image,
};
use vision::{
    image_segmenter::{self, ImageSegmenter},
    limb_projector::{self, LimbProjector},
};

#[derive(Debug, Deserialize)]
struct Metadata {
    camera_matrix: Option<CameraMatrix>,
    robot_kinematics: RobotKinematics,
    ground_to_field_of_home_after_coin_toss_before_second_half: Option<Isometry2<Ground, Field>>,
}

fn main() {
    for entry in read_dir("logs/images/VisionTop").unwrap() {
        let entry = entry.unwrap();
        if entry.path().extension().unwrap() != "png" {
            continue;
        }
        let png_path = entry.path();
        let json_path = png_path.with_extension("json");
        let output_path = png_path.with_extension("webp");

        let image = YCbCr422Image::load_from_444_png(png_path).unwrap();
        let metadata: Metadata = from_reader(File::open(json_path).unwrap()).unwrap();

        let mut image_segmenter_cycle_time = None;
        let field_color = FieldColor {
            function: FieldColorFunction::GreenChromaticity,
            red_chromaticity_threshold: 0.37,
            blue_chromaticity_threshold: 0.38,
            green_chromaticity_threshold: 0.5,
            green_luminance_threshold: 25.0,
            hue_low_threshold: 70.0,
            hue_high_threshold: 125.0,
            saturation_low_threshold: 100.0,
            saturation_high_threshold: 255.0,
            luminance_threshold: 25.0,
        };
        let horizontal_stride = 4;
        let vertical_stride = 2;

        let mut limb_projector = LimbProjector::new(limb_projector::CreationContext {}).unwrap();
        let mut image_segmenter = ImageSegmenter::new(image_segmenter::CreationContext {}).unwrap();

        let main_outputs = if metadata.camera_matrix.is_some() {
            limb_projector
                .cycle(limb_projector::CycleContext::new(
                    metadata.camera_matrix.as_ref().unwrap(),
                    &metadata.robot_kinematics,
                    &false,
                    &vec![],
                    &vec![],
                    &vec![],
                    &vec![],
                    &vec![],
                ))
                .unwrap()
        } else {
            limb_projector::MainOutputs::default()
        };
        let main_outputs = image_segmenter
            .cycle(image_segmenter::CycleContext::new(
                AdditionalOutput::new(false, &mut image_segmenter_cycle_time),
                &image,
                metadata.camera_matrix.as_ref(),
                metadata
                    .ground_to_field_of_home_after_coin_toss_before_second_half
                    .as_ref(),
                &field_color,
                main_outputs.projected_limbs.value.as_ref(),
                &horizontal_stride,
                &vertical_stride,
                &EdgeDetectionSourceParameters::Luminance,
                &Interpolated {
                    first_half_own_half_towards_own_goal: 20.0,
                    first_half_own_half_away_own_goal: 20.0,
                    first_half_opponent_half_towards_own_goal: 20.0,
                    first_half_opponent_half_away_own_goal: 20.0,
                },
                &MedianModeParameters::ThreePixels,
            ))
            .unwrap();

        let mut output_image = RgbImage::new(640, 480);
        for scan_line in main_outputs
            .image_segments
            .value
            .scan_grid
            .vertical_scan_lines
        {
            for segment in scan_line.segments {
                for x in (scan_line.position as u32)
                    ..(scan_line.position as u32 + horizontal_stride as u32)
                {
                    for y in (segment.start as u32)..(segment.end as u32) {
                        let mut rgb = color::Rgb::from(segment.color);
                        if segment.field_color == Intensity::High && x == scan_line.position as u32
                        {
                            rgb = color::Rgb::YELLOW;
                        }
                        *output_image.get_pixel_mut(x, y) = image::Rgb([rgb.r, rgb.g, rgb.b]);
                    }
                }
            }
        }
        output_image.save(output_path).unwrap();
    }
}
