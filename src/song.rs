use std::{io::Read, path::PathBuf, process::Command};

use anyhow::Result;
use egui::TextureHandle;
use serde_json::{json, Value};

use crate::app::{
    dbg_print_stderr, tempfile, FFMPEG_AUDIO_FORMAT, FFMPEG_AUDIO_FORMAT_EXT, FFMPEG_COMMAND,
};

#[derive(Default, Clone)]
pub struct Song {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub composer: String,

    pub audio_bytes: Vec<u8>,
    pub cover_bytes: Vec<u8>,

    pub source: String,

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
        let unwrap = |field: &str| {
            json.get(field)
                .unwrap_or(&json!(""))
                .to_string()
                .replace("\"", "")
        };
        self.title = unwrap("title");
        self.artist = unwrap("uploader");
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
