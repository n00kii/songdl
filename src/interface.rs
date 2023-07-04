use std::{io::Cursor, path::PathBuf, time::Duration};

use crate::{
    app::{self, App},
    song::{Origin, WAVEFORM_LENGTH},
};
use egui::{
    pos2, vec2, Align2, Button, CentralPanel, Color32, Context, FontData, FontFamily, FontId,
    Label, Layout, Rect, Response, RichText, Rounding, ScrollArea, Sense, Slider, Spinner, Stroke,
    Style, TextEdit, TopBottomPanel, Ui, Vec2,
};
use egui_extras::{Column, Size, StripBuilder, TableBuilder};
use kira::sound::static_sound::{StaticSoundData, StaticSoundSettings};

#[macro_export]
macro_rules! iconst {
    ($name:ident) => {
        crate::interface::constants::$name
    };
}

macro_rules! label {
    ($text:expr, $name:ident) => {
        format!("{}  {}", crate::interface::constants::$name, $text)
    };
}

#[derive(PartialEq, Default)]
pub enum InterfacePage {
    #[default]
    Downloader,
    Settings,
}

fn spacer(ui: &mut Ui) {
    ui.add_space(iconst!(SPACER_SIZE))
}

fn draw_nav_panel(app: &mut App, ctx: &Context) {
    TopBottomPanel::new(egui::panel::TopBottomSide::Top, "page_panel").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut app.current_page,
                InterfacePage::Downloader,
                label!("download", DOWNLOADER_ICON),
            );
            ui.selectable_value(
                &mut app.current_page,
                InterfacePage::Settings,
                label!("settings", SETTINGS_ICON),
            );
        });
    });
}

fn draw_settings(app: &mut App, ui: &mut Ui) {
    TableBuilder::new(ui)
        .column(Column::exact(150.))
        .column(Column::remainder())
        .header(iconst!(DETAILS_ROW_HEIGHT), |mut row| {
            row.col(|ui| {
                ui.label("field");
            });
            row.col(|ui| {
                ui.label("data");
            });
        })
        .body(|mut body| {
            let mut updated = false;
            fn path_field(
                body: &mut egui_extras::TableBody<'_>,
                label: &str,
                field_opt: &mut Option<String>,
                is_file: bool,
                updated: &mut bool,
            ) {
                body.row(iconst!(DETAILS_ROW_HEIGHT), |mut row| {
                    let mut dummy_string = String::new();
                    let mut field_enabled = field_opt.is_some();
                    let mut action = None;
                    let field = field_opt.as_mut().unwrap_or(&mut dummy_string);

                    let mut path = PathBuf::from(&field);

                    row.col(|ui| {
                        ui.horizontal(|ui| {
                            if ui.checkbox(&mut field_enabled, "").clicked() {
                                if field_enabled {
                                    action = Some(false);
                                } else {
                                    action = Some(true);
                                }
                                *updated = true;
                            }
                            ui.add_enabled_ui(field_enabled, |ui| {
                                ui.label(label);
                            });
                        });
                    });
                    row.col(|ui| {
                        ui.add_enabled_ui(field_enabled, |ui| {
                            ui.vertical_centered_justified(|ui| {
                                if path_edit(ui, &mut path, is_file).changed() {
                                    *field = pathbuf_to_string(&path);
                                    *updated = true;
                                }
                            });
                        });
                    });

                    if let Some(action) = action {
                        if action {
                            *field_opt = None;
                        } else {
                            *field_opt = Some(String::new());
                        }
                    }
                });
            }

            path_field(
                &mut body,
                "default save directory",
                &mut app.settings.default_save_directory,
                false,
                &mut updated,
            );
            path_field(
                &mut body,
                "ffmpeg location",
                &mut app.settings.ffmpeg_path,
                true,
                &mut updated,
            );
            path_field(
                &mut body,
                "yt-dl location",
                &mut app.settings.ytdl_path,
                true,
                &mut updated,
            );

            body.row(iconst!(DETAILS_ROW_HEIGHT), |mut row| {
                row.col(|ui| {
                    ui.label("playback volume");
                });
                row.col(|ui| {
                    if ui
                        .add(
                            Slider::new(&mut app.settings.playback_volume, 0.0..=1.0)
                                .custom_formatter(|v, _| format!("{}%", (v * 100.) as usize)),
                        )
                        .changed()
                    {
                        let _ = app.apply_playback_volume();
                    }
                });
            });

            if updated {
                app.read_config();
            }
        });
}

fn draw_cover_image(app: &mut App, ui: &mut Ui) {
    let image_size = [iconst!(COVER_SIZE); 2];
    if let Some(texture_handle) = app.downloader_state.song.cover_texture_handle.as_ref() {
        ui.image(texture_handle.id(), image_size);
    } else {
        let unk_cover_resp = ui.add_sized(
            image_size,
            Label::new(
                RichText::new(app.downloader_state.song_origin.to_string())
                    .size(iconst!(COVER_SIZE) * 0.15)
                    .color(iconst!(INACTIVE_FG_STROKE_COLOR)),
            )
            .sense(Sense::click()),
        );

        ui.painter().rect(
            unk_cover_resp.rect,
            Rounding::same(3.),
            Color32::TRANSPARENT,
            Stroke::new(1., iconst!(INACTIVE_FG_STROKE_COLOR)),
        );
    }
}

fn draw_options(app: &mut App, ui: &mut Ui) {
    ui.vertical_centered_justified(|ui| {
        ui.group(|ui| {
            ui.label("details");
            ui.separator();
            TableBuilder::new(ui)
                .auto_shrink([false, true])
                .min_scrolled_height(0.)
                .max_scroll_height(110.)
                .column(Column::exact(iconst!(DETAILS_ROW_HEIGHT)))
                .column(Column::auto().at_least(iconst!(DETAILS_LABEL_COLUMN_SIZE)))
                .column(Column::remainder())
                .column(Column::exact(0.))
                .body(|mut body| {
                    fn mk_row<T>(
                        body: &mut egui_extras::TableBody<'_>,
                        label: String,
                        f: impl FnOnce(&mut Ui) -> T,
                        enabled_opt: Option<&mut bool>,
                    ) {
                        let enabled = enabled_opt.is_none() || *enabled_opt.as_deref().unwrap();
                        body.row(iconst!(DETAILS_ROW_HEIGHT), |mut row| {
                            row.col(|ui| {
                                if let Some(enabled) = enabled_opt {
                                    ui.checkbox(enabled, "");
                                }
                            });
                            row.col(|ui| {
                                ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                                    ui.label(label);
                                });
                            });
                            row.col(|ui| {
                                ui.add_enabled_ui(enabled, |ui| f(ui));
                            });
                            row.col(|ui| {});
                        })
                    }

                    mk_row(
                        &mut body,
                        label!("title", DETAILS_TITLE_ICON),
                        |ui| ui.text_edit_singleline(&mut app.downloader_state.song.title),
                        None,
                    );
                    mk_row(
                        &mut body,
                        label!("artist", DETAILS_ARTIST_ICON),
                        |ui| ui.text_edit_singleline(&mut app.downloader_state.song.artist),
                        None,
                    );
                    mk_row(
                        &mut body,
                        label!("album", DETAILS_ALBUM_ICON),
                        |ui| ui.text_edit_singleline(&mut app.downloader_state.song.album),
                        Some(&mut app.downloader_state.separate_album),
                    );
                    mk_row(
                        &mut body,
                        label!("album artist", DETAILS_ALBUM_ARTIST_ICON),
                        |ui| ui.text_edit_singleline(&mut app.downloader_state.song.album_artist),
                        Some(&mut app.downloader_state.separate_album_artist),
                    );
                    mk_row(
                        &mut body,
                        label!("composer", DETAILS_COMPOSER_ICON),
                        |ui| ui.text_edit_singleline(&mut app.downloader_state.song.composer),
                        Some(&mut app.downloader_state.seperate_composer),
                    );
                    mk_row(
                        &mut body,
                        format!(
                            "{} ({}dB)",
                            label!("volume", VOLUME_ICON),
                            app.downloader_state.song.volume
                        ),
                        |ui| {
                            StripBuilder::new(ui)
                                .sizes(Size::remainder(), 2)
                                .horizontal(|mut strip| {
                                    strip.cell(|ui| {
                                        TextEdit::singleline(
                                            &mut app.downloader_state.volume_offset,
                                        )
                                        .hint_text("adjust volume (dB)...")
                                        .show(ui);
                                    });
                                    strip.cell(|ui| {
                                        if ui
                                            .add_enabled(
                                                app.downloader_state
                                                    .volume_offset
                                                    .parse::<f32>()
                                                    .is_ok(),
                                                Button::new("apply"),
                                            )
                                            .clicked()
                                        {
                                            app.apply_volume_offset();
                                        }
                                    });
                                })
                        },
                        None,
                    );
                });
        });
        ui.add_space(iconst!(SPACER_SIZE) * 5.);
        ui.group(|ui| {
            ui.label("save");
            ui.separator();
            path_edit(ui, &mut app.downloader_state.save_path, false);
            ui.add_enabled_ui(app.downloader_state.save_path.exists(), |ui| {
                if ui.button("write").clicked() {
                    app.save();
                }
            });
        });
    });
}

fn draw_downloader(app: &mut App, ui: &mut Ui) {
    ui.vertical_centered_justified(|ui| {
        spacer(ui);
        let tedit_response = TextEdit::singleline(&mut app.downloader_state.song.source_url)
            .hint_text("enter query url...")
            .horizontal_align(egui::Align::Center)
            .show(ui)
            .response;

        if tedit_response.changed() {
            app::remove_characters(&mut app.downloader_state.song.source_url, &["\""]);
            if !app.is_song_loading() {
                app.downloader_state.song_origin =
                    Origin::from_link(&app.downloader_state.song.source_url);
            }
        }

        if ui.button("query").clicked() {
            app.query(ui.ctx())
        };

        spacer(ui);
        ui.separator();
        spacer(ui);
    });

    let loading_spinner_rect = Rect::from_center_size(
        ui.available_rect_before_wrap().center(),
        Vec2::splat(iconst!(LOADING_SPINNER_SIZE)),
    );

    if !app.downloader_state.separate_album {
        app.downloader_state.song.album = app.downloader_state.song.title.clone();
    }
    if !app.downloader_state.separate_album_artist {
        app.downloader_state.song.album_artist = app.downloader_state.song.artist.clone();
    }
    if !app.downloader_state.seperate_composer {
        app.downloader_state.song.composer = app.downloader_state.song.artist.clone();
    }

    ui.add_enabled_ui(app.is_song_loaded() && !app.is_song_loading(), |ui| {
        StripBuilder::new(ui)
            .size(Size::remainder())
            .size(Size::exact(iconst!(SONG_BAR_HEIGHT)))
            .vertical(|mut strip| {
                strip.cell(|ui| {
                    StripBuilder::new(ui)
                        .size(Size::exact(0.))
                        .size(Size::exact(iconst!(COVER_SIZE) + iconst!(COVER_PADDING)))
                        .size(Size::remainder())
                        .horizontal(|mut strip| {
                            strip.empty();
                            strip.cell(|ui| {
                                draw_cover_image(app, ui);
                            });
                            strip.cell(|ui| {
                                draw_options(app, ui);
                            });
                        });
                });
                strip.cell(|ui| {
                    draw_waveform(app, ui);
                });
            });
    });

    if app.is_song_loading() {
        ui.put(
            loading_spinner_rect,
            Spinner::new().size(iconst!(LOADING_SPINNER_SIZE)),
        );
    }
}

fn pathbuf_to_string(path: &PathBuf) -> String {
    path.as_path().to_string_lossy().to_string()
}

fn path_edit(ui: &mut Ui, path: &mut PathBuf, is_file: bool) -> egui::Response {
    let mut tedit_response = TextEdit::singleline(&mut pathbuf_to_string(path))
        .hint_text("click to set path")
        .interactive(false)
        .show(ui)
        .response;

    if ui
        .interact(
            tedit_response.rect,
            tedit_response.id.with("click_sense"),
            Sense::click(),
        )
        .clicked()
    {
        if let Some(path_buf) = if is_file {
            rfd::FileDialog::new().pick_file()
        } else {
            rfd::FileDialog::new().pick_folder()
        } {
            *path = path_buf;
            tedit_response.mark_changed()
        }
    }
    tedit_response
}

pub fn draw_root(app: &mut App, ctx: &Context) {
    draw_nav_panel(app, ctx);

    CentralPanel::default().show(ctx, |ui| match app.current_page {
        InterfacePage::Downloader => draw_downloader(app, ui),
        InterfacePage::Settings => draw_settings(app, ui),
    });
}

fn draw_waveform(app: &mut App, ui: &mut Ui) -> Response {
    let widget_response = ui.allocate_response(
        vec2(ui.available_size_before_wrap().x, iconst!(SONG_BAR_HEIGHT)),
        Sense::focusable_noninteractive(),
    );

    let action_icon = match app.downloader_state.song_handle.as_ref().map(|h| h.state()) {
        Some(kira::sound::PlaybackState::Playing) => iconst!(PAUSE_ICON),
        Some(kira::sound::PlaybackState::Pausing) => iconst!(PLAY_ICON),
        Some(kira::sound::PlaybackState::Paused) => iconst!(PLAY_ICON),
        Some(kira::sound::PlaybackState::Stopping) => iconst!(PLAY_ICON),
        Some(kira::sound::PlaybackState::Stopped) => iconst!(PLAY_ICON),
        None => iconst!(STOP_ICON),
    };

    let icon_size = 16.;

    let mut icon_font_id = FontId::default();
    icon_font_id.size = icon_size;

    let icon_padding = 8.;
    let icon_color = iconst!(INACTIVE_FG_STROKE_COLOR);
    let action_icon_pos = widget_response.rect.left_center() + vec2(icon_padding, 0.);

    let icon_rect = ui.painter().text(
        action_icon_pos,
        Align2::LEFT_CENTER,
        action_icon,
        icon_font_id,
        icon_color,
    );

    let icon_response = ui.allocate_rect(icon_rect, Sense::click());

    let mut audio_rect = widget_response.rect;

    audio_rect.set_top(audio_rect.top() + icon_padding / 2.);
    audio_rect.set_bottom(audio_rect.bottom() - icon_padding / 2.);
    audio_rect.set_left(icon_size * 2. + icon_padding);
    audio_rect.set_right(widget_response.rect.right() - icon_padding);

    let waveform_response = ui.allocate_rect(audio_rect, Sense::click_and_drag());

    let hover_ratio = (app
        .downloader_state
        .song_handle
        .as_ref()
        .is_some_and(|h| h.state() != kira::sound::PlaybackState::Stopped)
        && ui.is_enabled())
    .then_some(ui.ctx().pointer_hover_pos().and_then(|p| {
        audio_rect.contains(p).then_some(
            ((p.x - audio_rect.left()) / audio_rect.width())
                .min(1.)
                .max(0.),
        )
    }))
    .flatten();

    let bar_paddding = 2.;
    let total_width = audio_rect.width();
    let bar_width = ((total_width - (WAVEFORM_LENGTH as f32 - 1.) * bar_paddding)
        / WAVEFORM_LENGTH as f32)
        .trunc();

    let mut next_bar_offset = audio_rect.left();
    let playback_position = app.song_position_ratio().unwrap_or_default();
    let painter = ui.painter();

    ui.ctx().tessellation_options_mut(|t| t.feathering = false);

    let empty_color = iconst!(WAVEFORM_EMPTY_COLOR);
    let filled_color = iconst!(WAVEFORM_FILLED_COLOR);

    let delta_weak_color = mix_colors(empty_color, filled_color, 0.2);
    let delta_strong_color = mix_colors(empty_color, filled_color, 0.5);

    app.downloader_state
        .song
        .waveform
        .0
        .iter()
        .enumerate()
        .for_each(|(i, s)| {
            let mut bar_rect = Rect::from_two_pos(
                pos2(next_bar_offset, audio_rect.top()),
                pos2(next_bar_offset + bar_width, audio_rect.bottom()),
            );
            let previous_bar_position = i as f32 / WAVEFORM_LENGTH as f32;
            let bar_position = (i + 1) as f32 / WAVEFORM_LENGTH as f32;

            let gamma = ((playback_position - previous_bar_position)
                / (bar_position - previous_bar_position))
                .min(1.)
                .max(0.) as f32;

            let bar_color = if let Some(hover_ratio) = hover_ratio {
                if hover_ratio > previous_bar_position as f32 {
                    mix_colors(delta_weak_color, filled_color, gamma)
                } else {
                    mix_colors(empty_color, delta_strong_color, gamma)
                }
            } else {
                mix_colors(empty_color, filled_color, gamma)
            };

            let bar_center = bar_rect.center();
            bar_rect.set_height((*s as f32 * audio_rect.height()).max(2.));
            bar_rect.set_center(bar_center);

            painter.rect_filled(bar_rect, Rounding::none(), bar_color);
            next_bar_offset += bar_width + bar_paddding;
        });

    ui.ctx().tessellation_options_mut(|t| t.feathering = true);

    if waveform_response.clicked() {
        if let Some(hover_ratio) = hover_ratio {
            let _ = app.seek_song(hover_ratio);
        }
    }

    if icon_response.clicked() {
        let _ = app.toggle_song_playback();
    }
    widget_response
}

pub fn load_style(ctx: &Context) {
    let mut style = Style::default();
    fn stroke(color: Color32) -> Stroke {
        Stroke::new(1., color)
    }
    style.visuals.widgets.noninteractive.bg_stroke = stroke(scale_color(iconst!(PRIMARY_BG_FILL_COLOR), 1.5));
    style.visuals.widgets.noninteractive.bg_fill = iconst!(PRIMARY_BG_FILL_COLOR);
    style.visuals.window_fill = iconst!(PRIMARY_BG_FILL_COLOR);
    style.visuals.panel_fill = iconst!(PRIMARY_BG_FILL_COLOR);
    
    style.visuals.extreme_bg_color = iconst!(SECONDARY_BG_FILL_COLOR);

    style.visuals.widgets.inactive.bg_fill = iconst!(INACTIVE_BG_FILL_COLOR);
    style.visuals.widgets.inactive.weak_bg_fill = iconst!(INACTIVE_BG_FILL_COLOR);

    style.visuals.widgets.inactive.fg_stroke = stroke(iconst!(INACTIVE_FG_STROKE_COLOR));

    style.visuals.widgets.hovered.bg_fill = iconst!(HOVERED_BG_FILL_COLOR);
    style.visuals.widgets.hovered.weak_bg_fill = iconst!(HOVERED_BG_FILL_COLOR);

    style.visuals.widgets.hovered.bg_stroke = stroke(iconst!(HOVERED_BG_STROKE_COLOR));
    style.visuals.widgets.hovered.fg_stroke = stroke(iconst!(HOVERED_FG_STROKE_COLOR));

    style.visuals.widgets.active.bg_fill = iconst!(ACTIVE_BG_FILL_COLOR);
    style.visuals.widgets.active.weak_bg_fill = iconst!(ACTIVE_BG_FILL_COLOR);

    style.visuals.widgets.active.bg_stroke = stroke(iconst!(ACTIVE_BG_STROKE_COLOR));
    style.visuals.widgets.active.fg_stroke = stroke(iconst!(ACTIVE_FG_STROKE_COLOR));

    style.visuals.selection.stroke = stroke(iconst!(SELECTED_FG_STROKE_COLOR));
    style.visuals.selection.bg_fill = iconst!(SELECTED_BG_FILL_COLOR);

    Stroke::default();

    ctx.set_style(style)
}

pub fn load_fonts(ctx: &Context) {
    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts);

    let mut phosphor_data = fonts.font_data.get_mut("phosphor").unwrap();
    phosphor_data.tweak = egui::FontTweak {
        y_offset: 1.25,
        ..Default::default()
    };

    fonts.font_data.insert(
        String::from("japanese_fallback"),
        FontData::from_static(include_bytes!("resources/NotoSansJP-Regular.otf")),
    );
    fonts.font_data.insert(
        String::from("korean_fallback"),
        FontData::from_static(include_bytes!("resources/NotoSansKR-Regular.otf")),
    );
    fonts.font_data.insert(
        String::from("s_chinese_fallback"),
        FontData::from_static(include_bytes!("resources/NotoSansSC-Regular.otf")),
    );
    fonts.font_data.insert(
        String::from("t_chinese_fallback"),
        FontData::from_static(include_bytes!("resources/NotoSansTC-Regular.otf")),
    );

    [
        "japanese_fallback",
        "korean_fallback",
        "s_chinese_fallback",
        "t_chinese_fallback",
    ]
    .into_iter()
    .for_each(|font_name| {
        fonts
            .families
            .get_mut(&FontFamily::Proportional)
            .unwrap()
            .push(String::from(font_name));
    });

    ctx.set_fonts(fonts);
}

fn mix_colors(a: Color32, b: Color32, gamma: f32) -> Color32 {
    let m = |a, b| (a as f32 * (1. - gamma) + b as f32 * gamma) as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}
fn scale_color(a: Color32, gamma: f32) -> Color32 {
    let m = |a| (a as f32 * gamma) as u8;
    Color32::from_rgb(m(a.r()), m(a.g()), m(a.b()))
}

pub mod constants {
    use egui::{vec2, Color32, Vec2};

    pub const DOWNLOADER_ICON: &str = "📥";
    pub const SETTINGS_ICON: &str = "⛭";
    pub const PLAY_ICON: &str = "▶";
    pub const PAUSE_ICON: &str = "⏸";
    pub const STOP_ICON: &str = "⏹";
    pub const YOUTUBE_ICON: &str = egui_phosphor::YOUTUBE_LOGO;
    pub const SOUNDCLOUD_ICON: &str = egui_phosphor::SOUNDCLOUD_LOGO;
    pub const FOLDER_ICON: &str = egui_phosphor::FOLDER;
    pub const VOLUME_ICON: &str = egui_phosphor::SPEAKER_SIMPLE_HIGH;

    pub const SPACER_SIZE: f32 = 5.;
    pub const DETAILS_ROW_HEIGHT: f32 = 20.;
    pub const DETAILS_LABEL_COLUMN_SIZE: f32 = 100.;
    pub const COVER_SIZE: f32 = 256.;
    pub const COVER_PADDING: f32 = 10.;
    pub const LOADING_SPINNER_SIZE: f32 = 15.;

    pub const SONG_BAR_HEIGHT: f32 = 35.;

    pub const DETAILS_TITLE_ICON: &str = egui_phosphor::TEXT_T;
    pub const DETAILS_ARTIST_ICON: &str = egui_phosphor::USER;
    pub const DETAILS_ALBUM_ICON: &str = egui_phosphor::IMAGES_SQUARE;
    pub const DETAILS_ALBUM_ARTIST_ICON: &str = egui_phosphor::USER_PLUS;
    pub const DETAILS_COMPOSER_ICON: &str = egui_phosphor::USER_GEAR;

    pub const WINDOW_SIZE: Vec2 = vec2(750., 375. + SONG_BAR_HEIGHT);

    pub const PRIMARY_BG_FILL_COLOR: Color32 = Color32::from_rgb(35, 38, 53);
    pub const SECONDARY_BG_FILL_COLOR: Color32 = Color32::from_rgb(28, 31, 43);

    pub const INACTIVE_FG_STROKE_COLOR: Color32 = Color32::from_rgb(103, 110, 149);
    pub const INACTIVE_BG_FILL_COLOR: Color32 = Color32::from_rgb(41, 45, 62);
    pub const HOVERED_BG_FILL_COLOR: Color32 = Color32::from_rgb(33, 37, 50);
    pub const HOVERED_BG_STROKE_COLOR: Color32 = Color32::from_rgb(103, 110, 149);
    pub const HOVERED_FG_STROKE_COLOR: Color32 = Color32::from_rgb(166, 172, 205);
    pub const ACTIVE_BG_FILL_COLOR: Color32 = Color32::from_rgb(33, 37, 50);
    pub const ACTIVE_BG_STROKE_COLOR: Color32 = Color32::from_rgb(128, 203, 196);
    pub const ACTIVE_FG_STROKE_COLOR: Color32 = Color32::from_rgb(128, 203, 196);
    pub const SELECTED_FG_STROKE_COLOR: Color32 = Color32::from_rgb(128, 203, 196);
    pub const SELECTED_BG_FILL_COLOR: Color32 = Color32::from_rgb(28, 31, 43);
    pub const ACCENT_COLOR: Color32 = Color32::from_rgb(128, 203, 196);

    pub const WAVEFORM_EMPTY_COLOR: Color32 = Color32::from_rgb(90, 100, 120);
    pub const WAVEFORM_FILLED_COLOR: Color32 = ACCENT_COLOR;
}
