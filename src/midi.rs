use xsynth_core::channel::{ChannelAudioEvent, ChannelEvent, ControlEvent};
use xsynth_core::channel_group::SynthEvent;
use crate::engine::SynthEngine;
use crate::params::TaiyangParams;

pub fn handle_note_event(
    event: nih_plug::prelude::NoteEvent<()>,
    engine: &mut SynthEngine,
    params: &TaiyangParams,
) {
    match event {
        nih_plug::prelude::NoteEvent::NoteOn { note, velocity, .. } => {
            let vel = (velocity * 127.0).clamp(0.0, 127.0) as u8;
            engine.send_event(SynthEvent::Channel(0, ChannelEvent::Audio(
                ChannelAudioEvent::NoteOn { key: note, vel }
            )));
        }
        nih_plug::prelude::NoteEvent::NoteOff { note, .. } => {
            engine.send_event(SynthEvent::Channel(0, ChannelEvent::Audio(
                ChannelAudioEvent::NoteOff { key: note }
            )));
        }
        nih_plug::prelude::NoteEvent::MidiCC { cc, value, .. } => {
            let val = (value * 127.0).clamp(0.0, 127.0) as u8;
            match cc {
                0 | 32 | 6 | 38 | 98 | 99 | 100 | 101 => {}
                _ => {
                    engine.update_cc(cc, val);
                    engine.send_event(SynthEvent::Channel(0, ChannelEvent::Audio(
                        ChannelAudioEvent::Control(ControlEvent::Raw(cc, val))
                    )));
                }
            }
        }
        nih_plug::prelude::NoteEvent::MidiPitchBend { value, .. } => {
            let normalized = (value - 0.5) * 2.0;
            engine.update_pb(normalized);
            engine.send_event(SynthEvent::Channel(0, ChannelEvent::Audio(
                ChannelAudioEvent::Control(ControlEvent::PitchBendValue(normalized))
            )));
        }
        nih_plug::prelude::NoteEvent::MidiProgramChange { program, .. } => {
            if !params.preset_locked.value() {
                engine.send_event(SynthEvent::Channel(0, ChannelEvent::Audio(
                    ChannelAudioEvent::ProgramChange(program)
                )));
            }
        }
        _ => {}
    }
}
