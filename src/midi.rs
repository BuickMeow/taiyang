use nih_plug::prelude::*;
use xsynth_core::channel::{ChannelAudioEvent, ChannelEvent, ControlEvent};
use xsynth_core::channel_group::SynthEvent;
use crate::engine::SynthEngine;
use crate::params::TaiyangParams;

pub fn handle_note_event(
    event: NoteEvent<()>,
    target_channel: u8,
    engine: &mut SynthEngine,
    params: &TaiyangParams,
) {
    let event_ch = match event {
        NoteEvent::NoteOn { channel, .. } => Some(channel),
        NoteEvent::NoteOff { channel, .. } => Some(channel),
        NoteEvent::PolyPressure { channel, .. } => Some(channel),
        NoteEvent::MidiChannelPressure { channel, .. } => Some(channel),
        NoteEvent::MidiPitchBend { channel, .. } => Some(channel),
        NoteEvent::MidiCC { channel, .. } => Some(channel),
        NoteEvent::MidiProgramChange { channel, .. } => Some(channel),
        _ => None,
    };

    if let Some(ch) = event_ch {
        if ch != target_channel {
            return;
        }
    }

    match event {
        NoteEvent::NoteOn { note, velocity, .. } => {
            let vel = (velocity * 127.0).clamp(0.0, 127.0) as u8;
            engine.send_event(SynthEvent::Channel(0, ChannelEvent::Audio(
                ChannelAudioEvent::NoteOn { key: note, vel }
            )));
        }
        NoteEvent::NoteOff { note, .. } => {
            engine.send_event(SynthEvent::Channel(0, ChannelEvent::Audio(
                ChannelAudioEvent::NoteOff { key: note }
            )));
        }
        NoteEvent::MidiCC { cc, value, .. } => {
            let val = (value * 127.0).clamp(0.0, 127.0) as u8;
            engine.send_event(SynthEvent::Channel(0, ChannelEvent::Audio(
                ChannelAudioEvent::Control(ControlEvent::Raw(cc, val))
            )));
        }
        NoteEvent::MidiPitchBend { value, .. } => {
            engine.send_event(SynthEvent::Channel(0, ChannelEvent::Audio(
                ChannelAudioEvent::Control(ControlEvent::PitchBendValue(
                    value.clamp(-1.0, 1.0)
                ))
            )));
        }
        NoteEvent::MidiProgramChange { program, .. } => {
            if !params.preset_locked.value() {
                engine.send_event(SynthEvent::Channel(0, ChannelEvent::Audio(
                    ChannelAudioEvent::ProgramChange(program)
                )));
            }
        }
        _ => {}
    }
}
