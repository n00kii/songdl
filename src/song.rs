use std::{fmt::Display, io::Cursor, path::PathBuf};

use anyhow::Result;
use egui::TextureHandle;
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle, StaticSoundSettings};
use serde_json::Value;

use crate::{
    app::{self, json_read},
    command::{
        apply_volume_offset, get_average_volume, write_cover_to_audio, write_metadata_to_audio,
        FFMPEG_AUDIO_FORMAT_EXT,
    },
    iconst,
};

#[derive(Default, Clone, Copy, PartialEq)]
pub enum Origin {
    YouTube,
    Soundcloud,
    Local,

    #[default]
    Unknown,
}

impl Origin {
    fn link_component(&self) -> &str {
        match self {
            Self::YouTube => "youtube.",
            Self::Soundcloud => "soundcloud.",
            _ => "",
        }
    }
    pub fn from_link(link: &String) -> Self {
        let contains_origin =
            |origin: Origin| -> bool { link.find(&origin.link_component()).is_some() };

        if contains_origin(Origin::YouTube) {
            Origin::YouTube
        } else if contains_origin(Origin::Soundcloud) {
            Origin::Soundcloud
        } else if PathBuf::from(link).exists() {
            Origin::Local
        } else {
            Origin::Unknown
        }
    }
}

impl Display for Origin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::YouTube => write!(f, "{}", iconst!(YOUTUBE_ICON)),
            Self::Soundcloud => write!(f, "{}", iconst!(SOUNDCLOUD_ICON)),
            Self::Local => write!(f, "{}", iconst!(FOLDER_ICON)),
            _ => write!(f, "?"),
        }
    }
}

#[derive(Default, Clone)]
pub struct Song {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub composer: String,

    pub audio_bytes: Vec<u8>,
    pub cover_bytes: Vec<u8>,

    pub source_url: String,
    pub volume: f32,

    pub cover_texture_handle: Option<TextureHandle>,
    pub audio_frames: Option<StaticSoundData>,
    pub waveform: Waveform,
}

pub const WAVEFORM_LENGTH: usize = 230;
#[derive(Clone)]
pub struct Waveform(pub [f32; WAVEFORM_LENGTH]);

impl Default for Waveform {
    fn default() -> Self {
        Self([0.; WAVEFORM_LENGTH])
    }
}

impl Waveform {
    pub fn new(values: Vec<f32>) -> Self {
        Self(values.try_into().unwrap_or([0.; WAVEFORM_LENGTH]))
    }
}

impl Song {
    fn trim(&mut self) {
        self.title = self.title.trim().to_string();
        self.artist = self.artist.trim().to_string();
        self.album = self.album.trim().to_string();
    }
    fn generate_metadata_tuples(&mut self) -> Vec<(String, String)> {
        self.trim();
        vec![
            (String::from("title"), self.title.clone()),
            (String::from("artist"), self.artist.clone()),
            (String::from("album"), self.album.clone()),
        ]
    }
    pub fn update_current_volume(&mut self) -> Result<()> {
        self.volume = get_average_volume(&self.audio_bytes)?;
        Ok(())
    }
    pub fn apply_volume_offset(&mut self, offset: f32) -> Result<()> {
        self.audio_bytes = apply_volume_offset(&self.audio_bytes, offset)?;
        self.update_current_volume()?;
        self.update_audio_frames()?;
        Ok(())
    }
    pub fn update_audio_frames(&mut self) -> Result<()> {
        let f_max = |f: &[f32]| f.iter().cloned().fold(f32::NAN, f32::max);

        let audio_frames = StaticSoundData::from_cursor(
            Cursor::new(self.audio_bytes.clone()),
            StaticSoundSettings::default(),
        )?;

        let mono_frames = audio_frames
            .frames
            .iter()
            .map(|f| (f.left as f32 + f.right as f32) * 0.5)
            .collect::<Vec<_>>();
        let num_chunks = mono_frames.len() / WAVEFORM_LENGTH;
        let mut waveform = mono_frames
            .chunks_exact(num_chunks)
            .map(|c| f_max(c))
            .collect::<Vec<_>>();
        let max = f_max(&waveform);
        waveform.iter_mut().for_each(|s: &mut f32| *s = *s / max);

        self.audio_frames = Some(audio_frames);
        self.waveform = Waveform::new(waveform);
        Ok(())
    }
    pub fn update_metadata_from_json(&mut self, json: Value) {
        if let serde_json::Value::Object(mut json) = json {
            [
                "requested_formats",
                "thumbnails",
                "url",
                "urls",
                "fragments",
                "formats",
                "automatic_captions",
            ]
            .into_iter()
            .for_each(|f| {
                json.remove(f);
            });

            let json = Value::Object(json);

            let set_if_exists = |struct_field: &mut String, json_field: &str| {
                let value = json_read(&json, json_field);
                if !value.is_empty() {
                    *struct_field = value;
                }
            };

            set_if_exists(&mut self.title, "title");
            set_if_exists(&mut self.artist, "artist");
            set_if_exists(&mut self.artist, "uploader");
        }
    }
    pub fn write_to_disk(&self, save_path: &PathBuf) -> Result<()> {
        let mut filename = format!("{}_{}{}", self.title, self.artist, FFMPEG_AUDIO_FORMAT_EXT)
            .to_ascii_lowercase()
            .replace(" ", "_");

        app::remove_characters(&mut filename, &["/", "*", ":", "?", "\"", "<", ">", "|"]);

        let mut final_save_path = save_path.clone();

        final_save_path.push(filename);
        std::fs::write(final_save_path, &self.audio_bytes)?;
        Ok(())
    }
    pub fn update_bytes_from_metadata(&mut self) -> Result<()> {
        let metadata = self.generate_metadata_tuples();
        let audio_bytes_with_metadata = write_metadata_to_audio(&self.audio_bytes, metadata)?;
        let audio_bytes_with_cover =
            write_cover_to_audio(&audio_bytes_with_metadata, &self.cover_bytes)?;
        self.audio_bytes = audio_bytes_with_cover;
        Ok(())
    }
}
