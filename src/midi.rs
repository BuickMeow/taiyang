use nih_plug::prelude::*;
use xsynth_core::channel::{ChannelAudioEvent, ChannelEvent, ControlEvent};
use xsynth_core::channel_group::SynthEvent;
use crate::engine::SynthEngine;
use crate::params::TaiyangParams;

pub fn handle_note_event(
    event: NoteEvent<()>,
    engine: &mut SynthEngine,
    params: &TaiyangParams,
) {
    let channel = match event {
        NoteEvent::NoteOn { channel, .. } => channel,
        NoteEvent::NoteOff { channel, .. } => channel,
        NoteEvent::PolyPressure { channel, .. } => channel,
        NoteEvent::MidiChannelPressure { channel, .. } => channel,
        NoteEvent::MidiPitchBend { channel, .. } => channel,
        NoteEvent::MidiCC { channel, .. } => channel,
        NoteEvent::MidiProgramChange { channel, .. } => channel,
        _ => 0,
    } as u32;

    match event {
        NoteEvent::NoteOn { note, velocity, .. } => {
            let vel = (velocity * 127.0).clamp(0.0, 127.0) as u8;
            engine.send_event(SynthEvent::Channel(channel, ChannelEvent::Audio(
                ChannelAudioEvent::NoteOn { key: note, vel }
            )));
        }
        NoteEvent::NoteOff { note, .. } => {
            engine.send_event(SynthEvent::Channel(channel, ChannelEvent::Audio(
                ChannelAudioEvent::NoteOff { key: note }
            )));
        }
        NoteEvent::MidiCC { cc, value, .. } => {
            let val = (value * 127.0).clamp(0.0, 127.0) as u8;
            match cc {
                0 | 32 | 6 | 38 | 98 | 99 | 100 | 101 => {}
                7 => {
                    // Volume (CC7): 记录原始值 + 发送补偿后值
                    engine.update_vol_raw_and_compensate(channel, val);
                    engine.update_cc(channel, cc, val);
                }
                _ => {
                    engine.update_cc(channel, cc, val);
                    engine.send_event(SynthEvent::Channel(channel, ChannelEvent::Audio(
                        ChannelAudioEvent::Control(ControlEvent::Raw(cc, val))
                    )));
                }
            }
        }
        NoteEvent::MidiPitchBend { value, .. } => {
            let normalized = (value - 0.5) * 2.0;
            engine.update_pb(channel, normalized);
            engine.send_event(SynthEvent::Channel(channel, ChannelEvent::Audio(
                ChannelAudioEvent::Control(ControlEvent::PitchBendValue(normalized))
            )));
            engine.apply_pb_volume_comp(channel, normalized);
        }
        NoteEvent::MidiProgramChange { program, .. } => {
            if !params.preset_locked.value() {
                engine.send_event(SynthEvent::Channel(channel, ChannelEvent::Audio(
                    ChannelAudioEvent::ProgramChange(program)
                )));
            }
        }
        _ => {}
    }
}
