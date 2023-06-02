use std::{io::Read, path::PathBuf, process::Command, fmt::Display};

use anyhow::Result;
use egui::TextureHandle;
use serde_json::{json, Value};

use crate::app::{
    tempfile, FFMPEG_AUDIO_FORMAT, FFMPEG_AUDIO_FORMAT_EXT, FFMPEG_COMMAND, json_read,
};

#[derive(Default, Clone, Copy)]
pub enum Origin {
    YouTube,
    Soundcloud,
    
    #[default]
    Unknown
}

impl Origin {
    fn link_component(&self) -> &str {
        match self {
            Self::YouTube => "youtube.",
            Self::Soundcloud => "soundcloud.",
            Self::Unknown => ""
        }
    }
    pub fn from_link(link: &String) -> Self {
        let contains_origin= |origin: Origin| -> bool {
            link.find(&origin.link_component()).is_some()
        };

        if contains_origin(Origin::YouTube) {
            Origin::YouTube
        } else if contains_origin(Origin::Soundcloud) {
            Origin::Soundcloud
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
            Self::Unknown => write!(f, "?"),
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

            let json_read = |field: &str| {
                json_read(&json, field)
            };

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
        fn generate_args_from_metadata(
            filepath: String,
            metadata: Vec<(String, String)>,
        ) -> Vec<String> {
            let inner_args = metadata
                .into_iter()
                .flat_map(|(key, value)| vec!["-metadata".to_string(), format!("{key}={value}")])
                .collect::<Vec<_>>();
            vec![
                String::from("-i"),
                filepath,
                // flush current metadata to prevent corruption
                String::from("-map"),
                String::from("0:a"),
                String::from("-map_metadata"),
                String::from("-1"),
                // copy stream codec
                String::from("-c"),
                String::from("copy"),
            ]
            .into_iter()
            .chain(inner_args.into_iter())
            .chain(
                vec![
                    String::from("-f"),
                    String::from(FFMPEG_AUDIO_FORMAT),
                    String::from("-"),
                ]
                .into_iter(),
            )
            .collect::<Vec<_>>()
        }
        let (_audio_tfile, audio_tfilepath) = tempfile(&self.audio_bytes)?;

        let intermediate_output = Command::new(FFMPEG_COMMAND)
            .args(generate_args_from_metadata(
                audio_tfilepath,
                self.generate_metadata_tuples(),
            ))
            .output()?;

        let (_cover_tfile, cover_tfilepath) = tempfile(&self.cover_bytes)?;
        let (_intm_audio_tfile, intm_audio_tfilepath) = tempfile(&intermediate_output.stdout)?;
        let (mut fin_audio_tfile, fin_audio_tfilepath) = tempfile(&[])?;

        let _finished_output = Command::new(FFMPEG_COMMAND)
            .args([
                "-i",
                &intm_audio_tfilepath,
                "-i",
                &cover_tfilepath,
                "-map",
                "0:0",
                "-map",
                "1:0",
                "-id3v2_version",
                "3",
                "-metadata:s:v",
                "title=Album cover",
                "-metadata:s:v",
                "comment=Cover (front)",
                "-y",
                "-f",
                FFMPEG_AUDIO_FORMAT,
                &fin_audio_tfilepath,
            ])
            .output()?;

        self.audio_bytes.clear();
        fin_audio_tfile.read_to_end(&mut self.audio_bytes)?;

        Ok(())
    }
}
