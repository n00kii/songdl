use crate::{
    command::{
        convert_audio, download_audio, download_thumbnail, extract_metadata, extract_thumbnail,
        get_average_volume, set_command, apply_volume_offset, DEFAULT_FFMPEG_COMMAND,
        DEFAULT_YT_DL_COMMAND,
    },
    iconst,
    interface::{self, InterfacePage, load_fonts, load_style},
    song::{Song, Waveform, WAVEFORM_LENGTH},
};

use anyhow::{bail, Context as ErrorContext, Result};
use eframe::{self, CreationContext};
use egui::{ColorImage, Context, FontData, FontFamily, TextureHandle, TextureOptions};
use egui_notify::{ToastOptions, ToastUpdate, Toasts};
use figment::{
    providers::{Format, Serialized},
    Figment,
};

use image::{imageops, DynamicImage};
use kira::{
    manager::{backend::DefaultBackend, AudioManager, AudioManagerSettings},
    sound::{
        static_sound::{StaticSoundData, StaticSoundHandle, StaticSoundSettings},
        PlaybackState,
    },
    tween::Tween,
};
use poll_promise::Promise;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Cursor, Write},
    path::PathBuf,
    time::Duration,
};

use crate::song::Origin;
use tempfile::NamedTempFile;

#[derive(Default)]
pub struct App {
    pub toasts: Toasts,
    pub current_page: InterfacePage,

    pub settings: Settings,
    pub downloader_state: DownloaderState,
    pub audio_manager: Option<AudioManager>,
}

pub const SETTINGS_FILENAME: &str = "settings.toml";

#[derive(Default)]
pub struct DownloaderState {
    pub song: Song,
    pub song_handle: Option<StaticSoundHandle>,
    pub song_origin: Origin,
    pub save_path: PathBuf,
    pub loading_song: Option<Promise<Result<Song>>>,

    pub volume_offset: String,

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

const PLAYBACK_TWEEN: Tween = Tween {
    duration: Duration::from_millis(200),
    start_time: kira::StartTime::Immediate,
    easing: kira::tween::Easing::Linear,
};

#[derive(Serialize, Deserialize, Default)]
pub struct Settings {
    pub default_save_directory: Option<String>,

    pub ffmpeg_path: Option<String>,
    pub ytdl_path: Option<String>,

    pub playback_volume: f32,
}

fn init_settings() -> Result<Settings> {
    Ok(Figment::from(Serialized::defaults(Settings::default()))
        .merge(figment::providers::Toml::file(SETTINGS_FILENAME))
        .extract()?)
}

pub fn json_read(json: &Value, field: &str) -> String {
    json.get(field)
        .unwrap_or(&json!(""))
        .to_string()
        .replace("\"", "")
}



impl eframe::App for App {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        interface::draw_root(self, ctx);
        self.toasts.show(ctx);
        self.update_state(ctx);
        ctx.request_repaint();
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Err(_error) = (|| {
            let toml_string = toml::to_string(&self.settings)?;
            fs::write(SETTINGS_FILENAME, toml_string)?;
            anyhow::Ok(())
        })() {}
    }
}

pub fn init() {
    let window_options = eframe::NativeOptions {
        initial_window_size: Some(iconst!(WINDOW_SIZE)),
        resizable: false,
        ..Default::default()
    };

    let mut app = App::default();
    let settings = init_settings().expect("failed to initialize settings");
    app.settings = settings;

    app.read_config();

    app.audio_manager = AudioManager::<DefaultBackend>::new(AudioManagerSettings::default()).ok();

    let _ = eframe::run_native(
        env!("CARGO_PKG_NAME"),
        window_options,
        Box::new(|cc| {
            let ctx = &cc.egui_ctx;
            load_fonts(ctx);
            load_style(ctx);
            Box::new(app)
        }),
    );
}

fn load_egui_image(ctx: &Context, name: &str, image: &DynamicImage) -> Result<TextureHandle> {
    let (w, h) = (image.width(), image.height());
    let image_cropped = imageops::crop_imm(
        image,
        if h > w { 0 } else { (w - h) / 2 },
        if w > h { 0 } else { (h - w) / 2 },
        if h > w { w } else { h },
        if w > h { h } else { w },
    )
    .to_image();
    let egui_image = ColorImage::from_rgba_unmultiplied(
        [
            image_cropped.width() as usize,
            image_cropped.height() as usize,
        ],
        image_cropped.as_flat_samples().as_slice(),
    );
    Ok(ctx.load_texture(name, egui_image, TextureOptions::default()))
}

pub fn tempfile(contents: &[u8]) -> Result<(NamedTempFile, String)> {
    let mut tempfile = tempfile::NamedTempFile::new()?;
    let path = tempfile.path().to_string_lossy().to_string();
    tempfile.write(contents)?;
    Ok((tempfile, path))
}

pub fn remove_characters(s: &mut String, c: &[&str]) {
    c.into_iter().for_each(|ss| {
        *s = s.replace(ss, "");
    });
}

impl App {
    pub fn start_song(&mut self) -> Result<()> {
        self.stop_current_playing_song()?;
        if let Some(audio_manager) = self.audio_manager.as_mut() {
            if let Some(sound_data) = self.downloader_state.song.audio_frames.clone() {
                let mut song_handle = audio_manager.play(sound_data)?;
                song_handle.set_volume(self.settings.playback_volume as f64, PLAYBACK_TWEEN)?;
                self.downloader_state.song_handle = Some(song_handle);
            }
        } else {
            bail!("no sound device")
        }
        Ok(())
    }
    pub fn apply_playback_volume(&mut self) -> Result<()> {
        if let Some(current_song_handle) = self.downloader_state.song_handle.as_mut() {
            current_song_handle.set_volume(self.settings.playback_volume as f64, Tween::default())?;
        }
        Ok(())
    }

    pub fn stop_current_playing_song(&mut self) -> Result<()> {
        if let Some(current_song_handle) = self.downloader_state.song_handle.as_mut() {
            current_song_handle.stop(Tween::default())?;
        }
        Ok(())
    }

    pub fn toggle_song_playback(&mut self) -> Result<()> {
        let mut do_start = false;
        if let Some(current_song_handle) = self.downloader_state.song_handle.as_mut() {
            match current_song_handle.state() {
                kira::sound::PlaybackState::Playing => current_song_handle.pause(PLAYBACK_TWEEN)?,
                kira::sound::PlaybackState::Pausing => {
                    current_song_handle.resume(PLAYBACK_TWEEN)?
                }
                kira::sound::PlaybackState::Paused => current_song_handle.resume(PLAYBACK_TWEEN)?,
                kira::sound::PlaybackState::Stopping => do_start = true,
                kira::sound::PlaybackState::Stopped => do_start = true,
            }
        } else {
            do_start = true;
        }
        if do_start {
            self.start_song()?;
        }
        Ok(())
    }

    pub fn seek_song(&mut self, seek_ratio: f32) -> Result<()> {
        let total_duration = self
            .downloader_state
            .song
            .audio_frames
            .as_ref()
            .map(|s| s.duration())
            .context("no song data")?;
        if let Some(current_song_handle) = self.downloader_state.song_handle.as_mut() {
            let target_position = total_duration.as_secs() as f32 * seek_ratio;
            current_song_handle.seek_to(target_position as f64)?;
        }
        Ok(())
    }

    pub fn song_position_ratio(&mut self) -> Option<f32> {
        self.downloader_state.song.audio_frames.as_ref().and_then(|d| {
            self.downloader_state
                .song_handle
                .as_ref()
                .map(|s| s.position() as f32 / d.duration().as_secs() as f32)
        })
    }

    fn update_state(&mut self, ctx: &Context) {
        if self.downloader_state.loading_song.is_ready() {
            let loaded_song = self.downloader_state.loading_song.unwrap_and_take();
            if let Ok(song) = loaded_song {
                self.downloader_state.song = song;
            }
        }
    }
    pub fn read_config(&mut self) {
        if let Some(default_save_directory) = self.settings.default_save_directory.as_ref() {
            self.downloader_state.save_path = PathBuf::from(default_save_directory);
        }

        set_command(DEFAULT_FFMPEG_COMMAND, self.settings.ffmpeg_path.clone());
        set_command(DEFAULT_YT_DL_COMMAND, self.settings.ytdl_path.clone());
    }
    pub fn is_song_loaded(&self) -> bool {
        !self.downloader_state.song.audio_bytes.is_empty()
    }
    pub fn is_song_loading(&self) -> bool {
        self.downloader_state.loading_song.is_some()
    }
    pub fn apply_volume_offset(&mut self) {
        let mut song = self.downloader_state.song.clone();
        let offset = self.downloader_state.volume_offset.parse::<f32>().unwrap();
        let toast = self.toasts.info("setting volume...").create_channel();
        let _ = self.stop_current_playing_song();
        self.downloader_state.loading_song = Some(Promise::spawn_thread("save_song", move || {
            if let Err(error) = (|| {
                song.apply_volume_offset(offset)?;
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
        let ctx_clone = ctx.clone();
        let query_url = self.downloader_state.song.source_url.clone();
        let song_origin = self.downloader_state.song_origin;
        let toast = self.toasts.info("initializing...").create_channel();

        let _ = self.stop_current_playing_song();

        self.downloader_state.loading_song = Some(Promise::spawn_thread("query_song", move || {
            let mut song: Song = Song::default();
            if let Err(error) = (|| {
                if song_origin == Origin::Local {
                    toast.send(ToastUpdate::caption("reading..."))?;
                    let audio_bytes = fs::read(&query_url)?;

                    if audio_bytes.is_empty() {
                        bail!("read error")
                    }

                    toast.send(ToastUpdate::caption("converting audio..."))?;
                    let converted_audio_bytes = convert_audio(&audio_bytes)?;

                    if converted_audio_bytes.is_empty() {
                        bail!("audio conversion error")
                    }

                    toast.send(ToastUpdate::caption("extracting thumbnail..."))?;
                    let cover_bytes = extract_thumbnail(&audio_bytes)?;

                    toast.send(ToastUpdate::caption("loading cover..."))?;
                    if !cover_bytes.is_empty() {
                        let image = image::load_from_memory(&cover_bytes)?;
                        let cover_texture_handle = load_egui_image(&ctx_clone, &song.title, &image)?;
                        song.cover_texture_handle = Some(cover_texture_handle);

                    }

                    toast.send(ToastUpdate::caption("parsing metadata..."))?;
                    let audio_details = extract_metadata(&audio_bytes)?;
                    song.update_metadata_from_json(audio_details);

                    song.cover_bytes = cover_bytes;
                    song.audio_bytes = converted_audio_bytes;
                    song.source_url = query_url;
                } else {
                    toast.send(ToastUpdate::caption("downloading audio..."))?;
                    let (audio_bytes, audio_details) = download_audio(&query_url)?;

                    if audio_bytes.is_empty() {
                        bail!("download error")
                    }

                    toast.send(ToastUpdate::caption("converting audio..."))?;
                    let converted_audio_bytes = convert_audio(&audio_bytes)?;

                    if converted_audio_bytes.is_empty() {
                        bail!("audio conversion error")
                    }

                    toast.send(ToastUpdate::caption("downloading thumbnail..."))?;
                    let image_output = download_thumbnail(&json_read(&audio_details, "thumbnail"))?;

                    toast.send(ToastUpdate::caption("parsing metadata..."))?;
                    song.update_metadata_from_json(audio_details);

                    let mut cover_bytes = vec![];

                    toast.send(ToastUpdate::caption("loading cover..."))?;
                    if !image_output.stdout.is_empty() {
                        let image = image::load_from_memory(&image_output.stdout)?;
                        let cover_texture_handle =
                            load_egui_image(&ctx_clone, &song.title, &image)?;
                        image.write_to(
                            &mut Cursor::new(&mut cover_bytes),
                            image::ImageFormat::Jpeg,
                        )?;
                        song.cover_texture_handle = Some(cover_texture_handle);
                    }

                    song.cover_bytes = cover_bytes;
                    song.audio_bytes = converted_audio_bytes;
                    song.source_url = query_url;
                }

                toast.send(ToastUpdate::caption("reading song..."))?;
                song.update_audio_frames()?;
                song.update_current_volume()?;

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
