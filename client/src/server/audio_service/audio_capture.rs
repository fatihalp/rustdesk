use hbb_common::anyhow::{bail, Result};

const STEREO_CHANNELS: usize = 2;

pub(super) struct CaptureFrameBuffer {
    samples: Vec<f32>,
    filled: usize,
}

impl CaptureFrameBuffer {
    pub(super) fn new(samples: usize) -> Result<Self> {
        if samples == 0 {
            bail!("Audio capture frame must contain at least one sample");
        }
        Ok(Self {
            samples: vec![0.0; samples],
            filled: 0,
        })
    }

    pub(super) fn process(
        &mut self,
        input: impl Iterator<Item = f32>,
        mut on_frame: impl FnMut(&[f32]),
    ) {
        for sample in input {
            self.samples[self.filled] = sample;
            self.filled += 1;
            if self.filled == self.samples.len() {
                self.filled = 0;
                on_frame(&self.samples);
            }
        }
    }
}

pub(super) fn rechannel<'a>(
    input: &'a [f32],
    channels: u16,
    output: &'a mut Vec<f32>,
) -> &'a [f32] {
    let input = if channels > STEREO_CHANNELS as u16 {
        &input[..input.len() / channels as usize * channels as usize]
    } else {
        input
    };
    output.clear();
    match channels {
        3 => rechannel_frame::<3>(input, output),
        4 => rechannel_frame::<4>(input, output),
        5 => rechannel_frame::<5>(input, output),
        6 => rechannel_frame::<6>(input, output),
        7 => rechannel_frame::<7>(input, output),
        8 => rechannel_frame::<8>(input, output),
        _ => return input,
    }
    output
}

fn rechannel_frame<const CHANNELS: usize>(input: &[f32], output: &mut Vec<f32>) {
    use fon::{
        chan::{Ch32, Channel},
        Frame,
    };

    for samples in input.chunks_exact(CHANNELS) {
        let mut frame = Frame::<Ch32, CHANNELS>::default();
        for (channel, sample) in frame.channels_mut().iter_mut().zip(samples) {
            *channel = (*sample).into();
        }
        let stereo = frame.to::<Ch32, CHANNELS>().to::<Ch32, STEREO_CHANNELS>();
        output.extend(stereo.channels().iter().map(|channel| channel.to_f32()));
    }
}
