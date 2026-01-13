//! Audio API for Lua
//!
//! 오디오 재생, 볼륨 제어 등

use mlua::{Lua, Result as LuaResult, Table};

/// Audio command from Lua
#[derive(Debug, Clone)]
pub enum AudioCommand {
    Play { sound: String, volume: f32, looping: bool },
    Play3D { sound: String, position: (f32, f32, f32), volume: f32, looping: bool },
    PlayMusic { sound: String },
    Stop { id: u64 },
    StopAll,
    StopMusic,
    Pause { id: u64 },
    Resume { id: u64 },
    SetMasterVolume { volume: f32 },
    SetMusicVolume { volume: f32 },
    SetSfxVolume { volume: f32 },
    SetSourcePosition { id: u64, position: (f32, f32, f32) },
}

/// Register Audio API
pub fn register_audio_api(lua: &Lua, skope: &Table) -> LuaResult<()> {
    let audio = lua.create_table()?;

    // Command queue for Rust to process
    let cmd_queue = lua.create_table()?;
    audio.set("_command_queue", cmd_queue)?;

    // Audio.play(sound_name, [volume], [loop])
    audio.set("play", lua.create_function(|lua, args: mlua::MultiValue| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let audio: Table = skope.get("Audio")?;
        let queue: Table = audio.get("_command_queue")?;
        let len = queue.len()? as i64;

        let mut iter = args.into_iter();
        let sound: String = iter.next()
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_default();
        let volume: f32 = iter.next()
            .and_then(|v| v.as_number().map(|n| n as f32))
            .unwrap_or(1.0);
        let looping: bool = iter.next()
            .and_then(|v| v.as_boolean())
            .unwrap_or(false);

        let cmd = lua.create_table()?;
        cmd.set("type", "play")?;
        cmd.set("sound", sound)?;
        cmd.set("volume", volume)?;
        cmd.set("loop", looping)?;

        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Audio.play_music(sound_name)
    audio.set("play_music", lua.create_function(|lua, sound: String| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let audio: Table = skope.get("Audio")?;
        let queue: Table = audio.get("_command_queue")?;
        let len = queue.len()? as i64;

        let cmd = lua.create_table()?;
        cmd.set("type", "play_music")?;
        cmd.set("sound", sound)?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Audio.stop(id) - stop specific sound
    audio.set("stop", lua.create_function(|lua, id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let audio: Table = skope.get("Audio")?;
        let queue: Table = audio.get("_command_queue")?;
        let len = queue.len()? as i64;

        let cmd = lua.create_table()?;
        cmd.set("type", "stop")?;
        cmd.set("id", id)?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Audio.stop_all()
    audio.set("stop_all", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let audio: Table = skope.get("Audio")?;
        let queue: Table = audio.get("_command_queue")?;
        let len = queue.len()? as i64;

        let cmd = lua.create_table()?;
        cmd.set("type", "stop_all")?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Audio.stop_music()
    audio.set("stop_music", lua.create_function(|lua, ()| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let audio: Table = skope.get("Audio")?;
        let queue: Table = audio.get("_command_queue")?;
        let len = queue.len()? as i64;

        let cmd = lua.create_table()?;
        cmd.set("type", "stop_music")?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Audio.set_volume(volume) - master volume
    audio.set("set_volume", lua.create_function(|lua, volume: f32| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let audio: Table = skope.get("Audio")?;
        let queue: Table = audio.get("_command_queue")?;
        let len = queue.len()? as i64;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_master_volume")?;
        cmd.set("volume", volume)?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Audio.set_music_volume(volume)
    audio.set("set_music_volume", lua.create_function(|lua, volume: f32| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let audio: Table = skope.get("Audio")?;
        let queue: Table = audio.get("_command_queue")?;
        let len = queue.len()? as i64;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_music_volume")?;
        cmd.set("volume", volume)?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Audio.set_sfx_volume(volume)
    audio.set("set_sfx_volume", lua.create_function(|lua, volume: f32| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let audio: Table = skope.get("Audio")?;
        let queue: Table = audio.get("_command_queue")?;
        let len = queue.len()? as i64;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_sfx_volume")?;
        cmd.set("volume", volume)?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Audio.play_3d(sound_name, position, [volume], [loop])
    audio.set("play_3d", lua.create_function(|lua, args: mlua::MultiValue| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let audio: Table = skope.get("Audio")?;
        let queue: Table = audio.get("_command_queue")?;
        let len = queue.len()? as i64;

        let mut iter = args.into_iter();
        let sound: String = iter.next()
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_default();
        let position = iter.next()
            .and_then(|v| v.as_table().cloned())
            .unwrap_or_else(|| lua.create_table().unwrap());
        let volume: f32 = iter.next()
            .and_then(|v| v.as_number().map(|n| n as f32))
            .unwrap_or(1.0);
        let looping: bool = iter.next()
            .and_then(|v| v.as_boolean())
            .unwrap_or(false);

        let cmd = lua.create_table()?;
        cmd.set("type", "play_3d")?;
        cmd.set("sound", sound)?;
        cmd.set("x", position.get::<f32>("x").unwrap_or(0.0))?;
        cmd.set("y", position.get::<f32>("y").unwrap_or(0.0))?;
        cmd.set("z", position.get::<f32>("z").unwrap_or(0.0))?;
        cmd.set("volume", volume)?;
        cmd.set("loop", looping)?;

        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Audio.pause(id)
    audio.set("pause", lua.create_function(|lua, id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let audio: Table = skope.get("Audio")?;
        let queue: Table = audio.get("_command_queue")?;
        let len = queue.len()? as i64;

        let cmd = lua.create_table()?;
        cmd.set("type", "pause")?;
        cmd.set("id", id)?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Audio.resume(id)
    audio.set("resume", lua.create_function(|lua, id: u64| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let audio: Table = skope.get("Audio")?;
        let queue: Table = audio.get("_command_queue")?;
        let len = queue.len()? as i64;

        let cmd = lua.create_table()?;
        cmd.set("type", "resume")?;
        cmd.set("id", id)?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    // Audio.set_source_position(id, position)
    audio.set("set_source_position", lua.create_function(|lua, (id, position): (u64, Table)| {
        let skope: Table = lua.globals().get("SKOPE")?;
        let audio: Table = skope.get("Audio")?;
        let queue: Table = audio.get("_command_queue")?;
        let len = queue.len()? as i64;

        let cmd = lua.create_table()?;
        cmd.set("type", "set_source_position")?;
        cmd.set("id", id)?;
        cmd.set("x", position.get::<f32>("x")?)?;
        cmd.set("y", position.get::<f32>("y")?)?;
        cmd.set("z", position.get::<f32>("z")?)?;
        queue.set(len + 1, cmd)?;
        Ok(())
    })?)?;

    skope.set("Audio", audio)?;
    Ok(())
}

/// Process audio commands from Lua
pub fn process_audio_commands(lua: &Lua) -> LuaResult<Vec<AudioCommand>> {
    let skope: Table = lua.globals().get("SKOPE")?;
    let audio: Table = skope.get("Audio")?;
    let queue: Table = audio.get("_command_queue")?;

    let mut commands = Vec::new();

    for pair in queue.pairs::<i64, Table>() {
        if let Ok((_, cmd)) = pair {
            let cmd_type: String = cmd.get("type").unwrap_or_default();

            let command = match cmd_type.as_str() {
                "play" => Some(AudioCommand::Play {
                    sound: cmd.get("sound").unwrap_or_default(),
                    volume: cmd.get("volume").unwrap_or(1.0),
                    looping: cmd.get("loop").unwrap_or(false),
                }),
                "play_music" => Some(AudioCommand::PlayMusic {
                    sound: cmd.get("sound").unwrap_or_default(),
                }),
                "stop" => Some(AudioCommand::Stop {
                    id: cmd.get("id").unwrap_or(0),
                }),
                "stop_all" => Some(AudioCommand::StopAll),
                "stop_music" => Some(AudioCommand::StopMusic),
                "set_master_volume" => Some(AudioCommand::SetMasterVolume {
                    volume: cmd.get("volume").unwrap_or(1.0),
                }),
                "set_music_volume" => Some(AudioCommand::SetMusicVolume {
                    volume: cmd.get("volume").unwrap_or(1.0),
                }),
                "set_sfx_volume" => Some(AudioCommand::SetSfxVolume {
                    volume: cmd.get("volume").unwrap_or(1.0),
                }),
                "play_3d" => Some(AudioCommand::Play3D {
                    sound: cmd.get("sound").unwrap_or_default(),
                    position: (
                        cmd.get("x").unwrap_or(0.0),
                        cmd.get("y").unwrap_or(0.0),
                        cmd.get("z").unwrap_or(0.0),
                    ),
                    volume: cmd.get("volume").unwrap_or(1.0),
                    looping: cmd.get("loop").unwrap_or(false),
                }),
                "pause" => Some(AudioCommand::Pause {
                    id: cmd.get("id").unwrap_or(0),
                }),
                "resume" => Some(AudioCommand::Resume {
                    id: cmd.get("id").unwrap_or(0),
                }),
                "set_source_position" => Some(AudioCommand::SetSourcePosition {
                    id: cmd.get("id").unwrap_or(0),
                    position: (
                        cmd.get("x").unwrap_or(0.0),
                        cmd.get("y").unwrap_or(0.0),
                        cmd.get("z").unwrap_or(0.0),
                    ),
                }),
                _ => None,
            };

            if let Some(c) = command {
                commands.push(c);
            }
        }
    }

    // Clear queue
    let new_queue = lua.create_table()?;
    audio.set("_command_queue", new_queue)?;

    Ok(commands)
}
