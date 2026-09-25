//! All filter tables and buffers are allocated before entering CPAL callbacks.
use avesra_contracts::ErrorCode;
use rubato::{
    Resampler, SincFixedOut, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
pub struct CaptureResampler {
    filter: SincFixedOut<f32>,
    input: Vec<Vec<f32>>,
    output: Vec<Vec<f32>>,
    used: usize,
    skip: usize,
}
impl CaptureResampler {
    pub fn new(rate: u32) -> Result<Self, ErrorCode> {
        if !(8000..=192000).contains(&rate) {
            return Err(ErrorCode::Unsupported);
        }
        let filter = SincFixedOut::<f32>::new(
            16000.0 / f64::from(rate),
            1.0,
            SincInterpolationParameters {
                sinc_len: 256,
                f_cutoff: 0.95,
                interpolation: SincInterpolationType::Cubic,
                oversampling_factor: 128,
                window: WindowFunction::BlackmanHarris2,
            },
            320,
            1,
        )
        .map_err(|_| ErrorCode::Unavailable)?;
        if filter.input_frames_max() > 8192 || filter.output_frames_max() != 320 {
            return Err(ErrorCode::Unsupported);
        }
        let input = filter.input_buffer_allocate(true);
        let output = filter.output_buffer_allocate(true);
        let skip = filter.output_delay();
        Ok(Self {
            filter,
            input,
            output,
            used: 0,
            skip,
        })
    }
    pub fn reset(&mut self) {
        self.filter.reset();
        self.input[0].fill(0.0);
        self.output[0].fill(0.0);
        self.used = 0;
        self.skip = self.filter.output_delay();
    }
    pub fn push(&mut self, sample: f32) -> Result<Option<&[f32]>, ErrorCode> {
        if self.used >= self.input[0].len() {
            return Err(ErrorCode::TooLarge);
        }
        self.input[0][self.used] = sample;
        self.used += 1;
        if self.used < self.filter.input_frames_next() {
            return Ok(None);
        }
        let (read, written) = self
            .filter
            .process_into_buffer(&self.input, &mut self.output, None)
            .map_err(|_| ErrorCode::Unavailable)?;
        if read != self.used || written != 320 {
            return Err(ErrorCode::Malformed);
        }
        self.used = 0;
        let skip = self.skip.min(written);
        self.skip -= skip;
        Ok(Some(&self.output[0][skip..written]))
    }
}
