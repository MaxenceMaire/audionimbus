/// States that an audio effect can be left in after processing a frame of audio.
///
/// Many audio effects use internal buffering and processing that causes audio to continue
/// outputting even after the input has stopped.
/// This remaining audio is called the "tail" and must be properly handled to avoid cutting off
/// reverb tails, echoes, and other time-based effects.
///
/// # Tail Workflow
///
/// 1. Apply the effect normally while audio is playing using `apply()`
/// 2. When input stops, call `tail()` repeatedly until it returns [`AudioEffectState::TailComplete`]
/// 3. Optionally check `tail_size()` to know how many samples remain
#[derive(Eq, PartialEq, Debug)]
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
