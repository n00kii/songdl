use std::{fmt::Display, path::PathBuf};

use anyhow::Result;
use egui::TextureHandle;
use serde_json::Value;

use crate::{
    app::json_read,
    command::{write_cover_to_audio, write_metadata_to_audio, FFMPEG_AUDIO_FORMAT_EXT},
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
            Self::YouTube => write!(f, "{}", egui_phosphor::YOUTUBE_LOGO),
            Self::Soundcloud => write!(f, "{}", egui_phosphor::SOUNDCLOUD_LOGO),
            Self::Local => write!(f, "{}", egui_phosphor::FOLDER),
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

    pub cover_texture_handle: Option<TextureHandle>,
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

            let json_read = |field: &str| json_read(&json, field);

            self.title = json_read("title");
            self.artist = json_read("uploader");
        }
    }
    pub fn write_to_disk(&self, save_path: &PathBuf) -> Result<()> {
        let filename = format!("{}_{}{}", self.title, self.artist, FFMPEG_AUDIO_FORMAT_EXT)
            .replace(" ", "_")
            .to_lowercase();
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
