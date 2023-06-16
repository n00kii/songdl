use std::path::PathBuf;

use crate::{app::App, song::Origin};
use egui::{
    CentralPanel, Color32, Context, Label, Layout, Rect, RichText, Rounding, Sense, Spinner,
    Stroke, TextEdit, TopBottomPanel, Ui, Vec2,
};
use egui_extras::{Column, Size, StripBuilder, TableBuilder};

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

fn settings(app: &mut App, ui: &mut Ui) {
    TableBuilder::new(ui)
        .striped(true)
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
            let mut row = |label: &str, field_opt: &mut Option<String>, is_file: bool| {
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
                                updated = true;
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
                                    updated = true;
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
            };

            row(
                "default save directory",
                &mut app.settings.default_save_directory,
                false,
            );
            row("ffmpeg location", &mut app.settings.ffmpeg_path, true);
            row("yt-dl location", &mut app.settings.ytdl_path, true);

            if updated {
                app.read_config();
            }
        });
}

fn downloader(app: &mut App, ui: &mut Ui) {
    ui.vertical_centered_justified(|ui| {
        spacer(ui);
        let tedit_response = TextEdit::singleline(&mut app.downloader_state.song.source_url)
            .hint_text("enter query url...")
            .horizontal_align(egui::Align::Center)
            .show(ui)
            .response;

        if !app.is_song_loading() && tedit_response.changed() {
            app.downloader_state.song_origin =
                Origin::from_link(&app.downloader_state.song.source_url);
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
            .size(Size::exact(0.))
            .size(Size::exact(iconst!(COVER_SIZE) + iconst!(COVER_PADDING)))
            .size(Size::remainder())
            .horizontal(|mut strip| {
                strip.empty();
                strip.cell(|ui| {
                    let image_size = [iconst!(COVER_SIZE); 2];
                    if let Some(texture_handle) =
                        app.downloader_state.song.cover_texture_handle.as_ref()
                    {
                        ui.image(texture_handle.id(), image_size);
                    } else {
                        let unk_cover_resp = ui.add_sized(
                            image_size,
                            Label::new(RichText::new(app.downloader_state.song_origin.to_string()).size(iconst!(COVER_SIZE) * 0.15))
                                .sense(Sense::click()),
                        );

                        ui.painter().rect(
                            unk_cover_resp.rect,
                            Rounding::same(3.),
                            Color32::TRANSPARENT,
                            Stroke::new(1., Color32::GRAY),
                        );
                    }
                });

                strip.cell(|ui| {
                    ui.vertical_centered_justified(|ui| {
                    ui.group(|ui| {
                        ui.label("details");
                        ui.separator();
                        TableBuilder::new(ui)
                        .auto_shrink([false, true])
                            .striped(true)
                            .column(Column::exact(iconst!(DETAILS_ROW_HEIGHT)))
                            .column(Column::auto().at_least(iconst!(DETAILS_LABEL_COLUMN_SIZE)))
                            .column(Column::remainder())
                            .body(|mut body| {
                                let mut mk_row = |label: String, field: &mut String, enabled_opt: Option<&mut bool>| {
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
                                                ui.add_enabled_ui(enabled, |ui| {
                                                        ui.text_edit_singleline(field);
                                                    });
                                                });

                                    })
                                };
                                mk_row(
                                    label!("title", DETAILS_TITLE_ICON),
                                    &mut app.downloader_state.song.title,
                                    None,
                                );
                                mk_row(
                                    label!("artist", DETAILS_ARTIST_ICON),
                                    &mut app.downloader_state.song.artist,
                                    None
                                );
                                mk_row(
                                    label!("album", DETAILS_ALBUM_ICON),
                                    &mut app.downloader_state.song.album,
                                    Some(&mut app.downloader_state.separate_album),
                                );
                                mk_row(
                                    label!("album artist", DETAILS_ALBUM_ARTIST_ICON),
                                    &mut app.downloader_state.song.album_artist,
                                    Some(&mut app.downloader_state.separate_album_artist)
                                );
                                mk_row(
                                    label!("composer", DETAILS_COMPOSER_ICON),
                                    &mut app.downloader_state.song.composer,
                                    Some(&mut app.downloader_state.seperate_composer),
                                );
                            });

                        });
                        ui.add_space(iconst!(SPACER_SIZE) * 5.);
                        ui.group(|ui| {
                            ui.label("save directory");
                            ui.separator();
                            path_edit(ui, &mut app.downloader_state.save_path, false);
                            ui.add_enabled_ui(app.downloader_state.save_path.exists(), |ui| {
                                if ui.button("write").clicked() {
                                    app.save();
                                }
                            });
                    });
                        });
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

pub fn root(app: &mut App, ctx: &Context) {
    draw_nav_panel(app, ctx);

    CentralPanel::default().show(ctx, |ui| match app.current_page {
        InterfacePage::Downloader => downloader(app, ui),
        InterfacePage::Settings => settings(app, ui),
    });
}

pub mod constants {
    use egui::{vec2, Vec2};

    pub const SPACER_SIZE: f32 = 5.;
    pub const DOWNLOADER_ICON: &str = "📥";
    pub const SETTINGS_ICON: &str = "⛭";
    pub const DETAILS_ROW_HEIGHT: f32 = 20.;
    pub const DETAILS_LABEL_COLUMN_SIZE: f32 = 100.;
    pub const COVER_SIZE: f32 = 256.;
    pub const COVER_PADDING: f32 = 10.;
    pub const LOADING_SPINNER_SIZE: f32 = 15.;

    pub const DETAILS_TITLE_ICON: &str = egui_phosphor::TEXT_AA;
    pub const DETAILS_ARTIST_ICON: &str = egui_phosphor::USER;
    pub const DETAILS_ALBUM_ICON: &str = egui_phosphor::CASSETTE_TAPE;
    pub const DETAILS_ALBUM_ARTIST_ICON: &str = egui_phosphor::USER_GEAR;
    pub const DETAILS_COMPOSER_ICON: &str = egui_phosphor::USERS;

    pub const WINDOW_SIZE: Vec2 = vec2(750., 375.);
}
