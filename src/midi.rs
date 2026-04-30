use nih_plug::prelude::*;
use xsynth_core::channel::{ChannelAudioEvent, ChannelEvent, ControlEvent};
use xsynth_core::channel_group::SynthEvent;
use crate::engine::SynthEngine;
use crate::params::TaiyangParams;

/// 检测事件是否来自 MIDI 通道 10（内部 0-indexed = 9）
pub fn is_channel_10(event: &NoteEvent<()>) -> bool {
    let ch = match event {
        NoteEvent::NoteOn { channel, .. } => Some(*channel),
        NoteEvent::NoteOff { channel, .. } => Some(*channel),
        NoteEvent::PolyPressure { channel, .. } => Some(*channel),
        NoteEvent::MidiChannelPressure { channel, .. } => Some(*channel),
        NoteEvent::MidiPitchBend { channel, .. } => Some(*channel),
        NoteEvent::MidiCC { channel, .. } => Some(*channel),
        NoteEvent::MidiProgramChange { channel, .. } => Some(*channel),
        _ => None,
    };
    ch == Some(9)
}

pub fn handle_note_event(
    event: NoteEvent<()>,
    engine: &mut SynthEngine,
    params: &TaiyangParams,
) {
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
            engine.update_cc(cc, val);
            engine.send_event(SynthEvent::Channel(0, ChannelEvent::Audio(
                ChannelAudioEvent::Control(ControlEvent::Raw(cc, val))
            )));
        }
        NoteEvent::MidiPitchBend { value, .. } => {
            // 兼容两种输入：原始 MIDI 值 (0~16383) 或已归一化值 (-1.0~1.0)
            let normalized = if value > 1.0 || value < -1.0 {
                (value - 8192.0) / 8192.0
            } else {
                value
            };
            engine.send_event(SynthEvent::Channel(0, ChannelEvent::Audio(
                ChannelAudioEvent::Control(ControlEvent::PitchBendValue(
                    normalized.clamp(-1.0, 1.0)
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
