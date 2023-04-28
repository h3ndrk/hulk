use std::time::Instant;

use alsa::{
    pcm::{Access, Format, HwParams},
    Direction, ValueOr, PCM,
};
use color_eyre::eyre::{Result, WrapErr};

fn main() -> Result<()> {
    let number_of_channels = 4;
    let number_of_samples = 4;
    let device =
        PCM::new("default", Direction::Capture, false).wrap_err("failed to open audio device")?;
    {
        let hardware_parameters =
            HwParams::any(&device).wrap_err("failed to create hardware parameters")?;
        hardware_parameters
            .set_access(Access::RWInterleaved)
            .wrap_err("failed to set access")?;
        hardware_parameters
            .set_format(Format::FloatLE)
            .wrap_err("failed to set format")?;
        hardware_parameters
            .set_rate_near(44100, ValueOr::Nearest)
            .wrap_err("failed to set sample rate")?;
        hardware_parameters
            .set_channels(4)
            .wrap_err("failed to set channel")?;
        device
            .hw_params(&hardware_parameters)
            .wrap_err("failed to set hardware parameters")?;
    }
    device.prepare().wrap_err("failed to prepare device")?;

    let now = Instant::now();
    loop {
        let io_device = device.io_f32().wrap_err("failed to create I/O device")?;
        let mut interleaved_buffer = vec![0.0; number_of_channels * number_of_samples];
        let number_of_frames = io_device
            .readi(&mut interleaved_buffer)
            .wrap_err("failed to read audio data")?;
        let mut non_interleaved_buffer: Vec<Vec<f32>> =
            vec![Vec::with_capacity(number_of_frames); number_of_channels];
        for (channel_index, non_interleaved_buffer) in non_interleaved_buffer.iter_mut().enumerate()
        {
            non_interleaved_buffer.extend(
                interleaved_buffer
                    .iter()
                    .skip(channel_index)
                    .step_by(number_of_channels),
            );
        }
        println!("{:?} {}", now.elapsed(), non_interleaved_buffer.len());
    }
}
