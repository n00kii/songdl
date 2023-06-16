use anyhow::Result;

use parking_lot::Mutex;
use serde_json::Value;
use std::{
    collections::HashMap,
    io::Read,
    os::windows::process::CommandExt,
    process::{Command, Output},
    sync::OnceLock,
};

use crate::app::tempfile;

pub const DEFAULT_YT_DL_COMMAND: &str = "yt-dlp";
pub const DEFAULT_FFMPEG_COMMAND: &str = "ffmpeg";
pub const DEFAULT_CURL_COMMAND: &str = "curl";

type CommandHashMap = Mutex<HashMap<&'static str, String>>;

fn command_map() -> &'static CommandHashMap {
    static MAP: OnceLock<CommandHashMap> = OnceLock::new();
    MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

pub const WIN_FLAG_CREATE_NO_WINDOW: u32 = 0x08000000;

pub const FFMPEG_AUDIO_FORMAT: &str = "mp3";
pub const FFMPEG_AUDIO_FORMAT_EXT: &str = ".mp3";

pub fn get_command(name: &str) -> String {
    command_map()
        .lock()
        .get(name)
        .map(|v| String::from(v))
        .unwrap_or(String::from(name))
}

pub fn set_command(name: &'static str, value: Option<String>) {
    if let Some(value) = value {
        command_map().lock().insert(name, value);
    } else {
        command_map().lock().remove(name);
    };
}

pub fn download_audio(query_url: &String) -> Result<(Vec<u8>, Value)> {
    let output = Command::new(get_command(DEFAULT_YT_DL_COMMAND))
        .args([
            "-j",
            "-f",
            "bestaudio",
            "--no-playlist",
            "--no-simulate",
            "--ignore-config",
            "--no-warnings",
            "-o",
            "-",
            &query_url,
        ])
        .creation_flags(WIN_FLAG_CREATE_NO_WINDOW)
        .output()?;

    Ok((output.stdout, serde_json::from_slice(&output.stderr)?))
}

pub fn convert_audio(audio_bytes: &[u8]) -> Result<Vec<u8>> {
    let (_audio_tfile, audio_tfilepath) = tempfile(audio_bytes)?;
    Ok(Command::new(get_command(DEFAULT_FFMPEG_COMMAND))
        .args(["-i", &audio_tfilepath, "-f", FFMPEG_AUDIO_FORMAT, "-"])
        .creation_flags(WIN_FLAG_CREATE_NO_WINDOW)
        .output()?
        .stdout)
}

pub fn download_thumbnail(query_url: &String) -> Result<Output> {
    Ok(Command::new(get_command(DEFAULT_CURL_COMMAND))
        .args([query_url, "-o", "-"])
        .creation_flags(WIN_FLAG_CREATE_NO_WINDOW)
        .output()?)
}

pub fn write_cover_to_audio(audio_bytes: &[u8], cover_bytes: &[u8]) -> Result<Vec<u8>> {
    let (_cover_tfile, cover_tfilepath) = tempfile(cover_bytes)?;
    let (_audio_tfile, audio_tfilepath) = tempfile(audio_bytes)?;
    let (mut final_audio_tfile, final_audio_tfilepath) = tempfile(&[])?;

    let mut final_audio_bytes = vec![];
    Command::new(get_command(DEFAULT_FFMPEG_COMMAND))
        .args([
            "-i",
            &audio_tfilepath,
            "-i",
            &cover_tfilepath,
            "-map",
            "0:0",
            "-map",
            "1:0",
            "-c",
            "copy",
            "-id3v2_version",
            "3",
            "-y",
            "-f",
            FFMPEG_AUDIO_FORMAT,
            &final_audio_tfilepath,
        ])
        .creation_flags(WIN_FLAG_CREATE_NO_WINDOW)
        .output()?;
    final_audio_tfile.read_to_end(&mut final_audio_bytes)?;
    Ok(final_audio_bytes)
}

pub fn write_metadata_to_audio(
    audio_bytes: &[u8],
    metadata: Vec<(String, String)>,
) -> Result<Vec<u8>> {
    let (_audio_tfile, audio_tfilepath) = tempfile(&audio_bytes)?;
    Ok(Command::new(get_command(DEFAULT_FFMPEG_COMMAND))
        .args(generate_args_from_metadata(audio_tfilepath, metadata))
        .creation_flags(WIN_FLAG_CREATE_NO_WINDOW)
        .output()?
        .stdout)
}

fn generate_args_from_metadata(filepath: String, metadata: Vec<(String, String)>) -> Vec<String> {
    let inner_args = metadata
        .into_iter()
        .flat_map(|(key, value)| vec!["-metadata".to_string(), format!("{key}={value}")])
        .collect::<Vec<_>>();
    vec![
        String::from("-i"),
        filepath,
        String::from("-map"),
        String::from("0:a"),
        String::from("-map_metadata"),
        String::from("-1"),
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
