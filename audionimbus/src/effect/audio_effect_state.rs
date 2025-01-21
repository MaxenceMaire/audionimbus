/// States that an audio effect can be left in after processing a frame of audio.
#[derive(Debug)]
pub enum AudioEffectState {
    /// One or more samples of tail remain in the effect’s internal buffers.
    TailRemaining,

    /// No tail remains in the effect’s internal buffers.
    TailComplete,
}

impl From<audionimbus_sys::IPLAudioEffectState> for AudioEffectState {
    fn from(state: audionimbus_sys::IPLAudioEffectState) -> Self {
        match state {
            audionimbus_sys::IPLAudioEffectState::IPL_AUDIOEFFECTSTATE_TAILREMAINING => {
                Self::TailRemaining
            }
            audionimbus_sys::IPLAudioEffectState::IPL_AUDIOEFFECTSTATE_TAILCOMPLETE => {
                Self::TailComplete
            }
        }
    }
}
