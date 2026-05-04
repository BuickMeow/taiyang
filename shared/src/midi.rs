use crate::engine::SynthEngine;
use xsynth_core::channel::{ChannelAudioEvent, ChannelEvent, ControlEvent};
use xsynth_core::channel_group::SynthEvent;

/// Handle a MIDI note event.
///
/// `channel_override`: if `Some(ch)`, all events are routed to channel `ch`
/// regardless of the MIDI channel in the event. Use this for single-channel
/// plugins. If `None`, the MIDI channel from the event is used directly
/// (for 16-channel plugins).
///
/// `min_velocity`: NoteOn events with velocity below this threshold are ignored.
/// Default should be 1 (velocity 0 = MIDI note-off convention).
pub fn handle_note_event(
    event: nih_plug::prelude::NoteEvent<()>,
    engine: &mut SynthEngine,
    preset_locked: bool,
    channel_override: Option<u8>,
    min_velocity: u8,
) {
    match event {
        nih_plug::prelude::NoteEvent::NoteOn {
            note,
            velocity,
            channel,
            ..
        } => {
            let ch = channel_override.unwrap_or(channel) as u32;
            let vel = (velocity * 127.0).clamp(0.0, 127.0) as u8;
            if (ch as u8) < engine.num_channels && vel > min_velocity {
                engine.send_event(SynthEvent::Channel(
                    ch,
                    ChannelEvent::Audio(ChannelAudioEvent::NoteOn { key: note, vel }),
                ));
            }
        }
        nih_plug::prelude::NoteEvent::NoteOff {
            note,
            velocity: _,
            channel,
            ..
        } => {
            let ch = channel_override.unwrap_or(channel) as u32;
            if (ch as u8) < engine.num_channels {
                engine.send_event(SynthEvent::Channel(
                    ch,
                    ChannelEvent::Audio(ChannelAudioEvent::NoteOff { key: note }),
                ));
            }
        }
        nih_plug::prelude::NoteEvent::MidiCC {
            cc, value, channel, ..
        } => {
            let ch = channel_override.unwrap_or(channel) as u32;
            if (ch as u8) < engine.num_channels {
                let val = (value * 127.0).clamp(0.0, 127.0) as u8;
                engine.send_event(SynthEvent::Channel(
                    ch,
                    ChannelEvent::Audio(ChannelAudioEvent::Control(ControlEvent::Raw(cc, val))),
                ));
            }
        }
        nih_plug::prelude::NoteEvent::MidiPitchBend { value, channel, .. } => {
            let ch = channel_override.unwrap_or(channel) as u32;
            if (ch as u8) < engine.num_channels {
                let normalized = (value - 0.5) * 2.0;
                engine.send_event(SynthEvent::Channel(
                    ch,
                    ChannelEvent::Audio(ChannelAudioEvent::Control(ControlEvent::PitchBendValue(
                        normalized,
                    ))),
                ));
            }
        }
        nih_plug::prelude::NoteEvent::MidiProgramChange {
            program, channel, ..
        } => {
            let ch = channel_override.unwrap_or(channel) as u32;
            if (ch as u8) < engine.num_channels && !preset_locked {
                engine.send_event(SynthEvent::Channel(
                    ch,
                    ChannelEvent::Audio(ChannelAudioEvent::ProgramChange(program)),
                ));
            }
        }
        _ => {}
    }
}
