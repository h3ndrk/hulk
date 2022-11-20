use context_attribute::context;
use framework::HardwareInterface;

pub struct MicrophoneRecorder {}

// #[context]
pub struct NewContext {}

// #[context]
pub struct CycleContext {
    pub hardware_interface: HardwareInterface,
}

// #[context]
#[derive(Default)]
pub struct MainOutputs {
    buffer: MainOutput<bool>,
}

impl MicrophoneRecorder {
    pub fn new(_context: NewContext) -> anyhow::Result<Self> {
        Ok(Self {})
    }

    pub fn cycle<Interface>(
        &mut self,
        context: CycleContext<Interface>,
    ) -> anyhow::Result<MainOutputs>
    where
        Interface: hardware::HardwareInterface,
    {
        Ok(MainOutputs::default())
        //hardware_interface
        //    .produce_audio_data()
        //    .context("Failed to record from the microphone")?;
        //Ok(hardware_interface.get_audio_buffer())
    }
}
