use crate::{
    iconst,
    interface::{self, InterfacePage},
    song::Song,
};
use anyhow::{bail, Result};
use eframe::{self, CreationContext};
use egui::{
    vec2, CentralPanel, ColorImage, Context, SidePanel, TextureHandle, TextureId, TextureOptions,
};
use egui_notify::{ToastOptions, ToastUpdate, Toasts};
use image::imageops;
use poll_promise::Promise;
use serde_json::{json, Value};
use std::{
    io::{BufReader, BufWriter, Cursor, IoSlice, Read, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
};
use tempfile::NamedTempFile;

#[derive(Default)]
pub struct App {
    pub toasts: Toasts,
    pub current_page: InterfacePage,
    pub downloader_state: DownloaderState,
}

pub const FFMPEG_AUDIO_FORMAT: &str = "mp3";
pub const FFMPEG_AUDIO_FORMAT_EXT: &str = ".mp3";
pub const VIDEO_FORMAT_CODE: i32 = 22;
pub const YT_DL_COMMAND: &str = "yt-dlp";
pub const FFMPEG_COMMAND: &str = "ffmpeg";
pub const COVER_CAPTURE_POSITION: i32 = 1;

#[derive(Default)]
pub struct DownloaderState {
    pub save_path: PathBuf,
    pub loading_song: Option<Promise<Result<Song>>>,
    pub save_progress: Option<Promise<Result<()>>>,
    pub song: Song,

    pub separate_album: bool,
    pub separate_album_artist: bool,
    pub seperate_composer: bool,
}

trait Ready {
    type Inner;
    fn unwrap_and_take(&mut self) -> Self::Inner;
    fn is_ready(&self) -> bool;
}

impl<T: Send> Ready for Option<Promise<T>> {
    type Inner = T;
    fn unwrap_and_take(&mut self) -> Self::Inner {
        self.take().unwrap().block_and_take()
    }
    fn is_ready(&self) -> bool {
        if let Some(promise) = self.as_ref() {
            promise.ready().is_some()
        } else {
            false
        }
    }
}

fn load_fonts(cc: &CreationContext) {
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts);

    let mut phosphor_data = fonts.font_data.get_mut("phosphor").unwrap();
    phosphor_data.tweak = egui::FontTweak {
        y_offset: 1.25,
        ..Default::default()
    };
    cc.egui_ctx.set_fonts(fonts);
}

impl eframe::App for App {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        interface::root(self, ctx);
        self.toasts.show(ctx);
        self.update_state();
    }
}

pub fn init() {
    let options = eframe::NativeOptions {
        initial_window_size: Some(iconst!(WINDOW_SIZE)),
        resizable: false,
        ..Default::default()
    };
    let _ = eframe::run_native(
        env!("CARGO_PKG_NAME"),
        options,
        Box::new(|cc| {
            load_fonts(cc);
            Box::new(App::default())
        }),
    );
}

fn load_egui_image(ctx: &Context, name: &str, image_bytes: &[u8]) -> Result<TextureHandle> {
    let image = image::load_from_memory(&image_bytes)?;
    let (w, h) = (image.width(), image.height());
    let image_cropped = imageops::crop_imm(
        &image,
        if h > w { 0 } else { (w - h) / 2 },
        if w > h { 0 } else { (h - w) / 2 },
        if h > w { w } else { h },
        if w > h { h } else { w },
    )
    .to_image();
    let egui_image = ColorImage::from_rgba_unmultiplied(
        [image_cropped.width() as usize, image_cropped.height() as usize],
        image_cropped.as_flat_samples().as_slice(),
    );
    Ok(ctx.load_texture(name, egui_image, TextureOptions::default()))
}

pub fn dbg_print_stderr(output: &std::process::Output) {
    println!(
        "{}",
        String::from_utf8(output.stderr.clone()).unwrap_or_default()
    );
}

pub fn tempfile(contents: &[u8]) -> Result<(NamedTempFile, String)> {
    let mut tempfile = tempfile::NamedTempFile::new()?;
    let path = tempfile.path().to_string_lossy().to_string();
    tempfile.write(contents)?;
    Ok((tempfile, path))
}

impl App {
    fn update_state(&mut self) {
        if self.downloader_state.loading_song.is_ready() {
            let loaded_song = self.downloader_state.loading_song.unwrap_and_take();
            match loaded_song {
                Ok(loaded_song) => {
                    self.downloader_state.song = loaded_song;
                }
                Err(_error) => {
                    // dbg!(error);
                }
            }
        }
    }
    pub fn is_song_loaded(&self) -> bool {
        !self.downloader_state.song.audio_bytes.is_empty()
    }
    pub fn is_song_loading(&self) -> bool {
        self.downloader_state.loading_song.is_some()
    }
    pub fn save(&mut self) {
        let mut song = self.downloader_state.song.clone();
        let save_path = self.downloader_state.save_path.clone();
        let toast = self.toasts.info("initializing...").create_channel();
        self.downloader_state.loading_song = Some(Promise::spawn_thread("save_song", move || {
            if let Err(error) = (|| {
                toast.send(ToastUpdate::caption("updating song metadata..."))?;
                song.update_bytes_from_metadata()?;
                toast.send(ToastUpdate::caption("writing song to disk..."))?;
                song.write_to_disk(&save_path)?;
                toast.send(
                    ToastUpdate::caption("saved")
                        .with_level(egui_notify::ToastLevel::Success)
                        .with_fallback_options(ToastOptions::default()),
                )?;
                anyhow::Ok(())
            })() {
                toast.send(
                    ToastUpdate::caption(format!("failed: {error}"))
                        .with_fallback_options(ToastOptions::default())
                        .with_level(egui_notify::ToastLevel::Error),
                )?;
                return Err(error);
            }
            Ok(song)
        }));
    }
    pub fn query(&mut self, ctx: &Context) {
        let query_url = self.downloader_state.song.source.clone();
        let toast = self.toasts.info("initializing...").create_channel();
        let ctx_clone = ctx.clone();
        self.downloader_state.loading_song = Some(Promise::spawn_thread("query_song", move || {
            let mut song: Song = Song::default();
            if let Err(error) = (|| {
                toast.send(ToastUpdate::caption("downloading..."))?;
                let video_output = Command::new(YT_DL_COMMAND)
                    .args([
                        "-j",
                        "-f",
                        &VIDEO_FORMAT_CODE.to_string(),
                        "--no-playlist",
                        "--no-simulate",
                        "--ignore-config",
                        "--no-warnings",
                        "-o",
                        "-",
                        &query_url,
                    ])
                    .output()?;

                if video_output.stdout.is_empty() {
                    bail!("failed to download")
                }

                toast.send(ToastUpdate::caption("writing..."))?;
                let (_video_tfile, video_tfilepath) = tempfile(&video_output.stdout)?;

                toast.send(ToastUpdate::caption("extracting audio..."))?;
                let audio_output = Command::new(FFMPEG_COMMAND)
                    .args([
                        "-i",
                        &video_tfilepath,
                        "-q:a",
                        "0",
                        "-map",
                        "a",
                        "-f",
                        FFMPEG_AUDIO_FORMAT,
                        "-",
                    ])
                    .output()?;

                toast.send(ToastUpdate::caption("extracting cover..."))?;
                let cover_output = Command::new(FFMPEG_COMMAND)
                    .args([
                        "-i",
                        &video_tfilepath,
                        "-ss",
                        &COVER_CAPTURE_POSITION.to_string(),
                        "-vframes",
                        "1",
                        "-q:v",
                        "2",
                        "-f",
                        "image2",
                        "-",
                    ])
                    .output()?;

                toast.send(ToastUpdate::caption("parsing metadata..."))?;
                let details_json: Value = serde_json::from_slice(&video_output.stderr)?;
                song.update_metadata_from_json(details_json);

                let cover_bytes = cover_output.stdout;
                let audio_bytes = audio_output.stdout;

                toast.send(ToastUpdate::caption("loading cover..."))?;
                let cover_texture_handle = load_egui_image(&ctx_clone, &song.title, &cover_bytes)?;

                song.cover_bytes = cover_bytes;
                song.cover_texture_handle = Some(cover_texture_handle);
                song.audio_bytes = audio_bytes;
                song.source = query_url.clone();

                anyhow::Ok(())
            })() {
                toast.send(
                    ToastUpdate::caption(format!("failed: {error}"))
                        .with_fallback_options(ToastOptions::default())
                        .with_level(egui_notify::ToastLevel::Error),
                )?;
                return Err(error);
            }
            Ok(song)
        }));
    }
}
