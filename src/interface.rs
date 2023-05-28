use std::path::PathBuf;

use crate::app::App;
use egui::{
    CentralPanel, Color32, Context, FontId, Label, Layout, Rect, RichText, Rounding, Sense,
    SidePanel, Spinner, Stroke, TextEdit, TopBottomPanel, Ui, Vec2, WidgetText,
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
    History,
    Settings,
}

fn spacer(ui: &mut Ui) {
    ui.add_space(iconst!(SPACER_SIZE))
}

pub fn draw_side_panel(app: &mut App, ctx: &Context) {
    TopBottomPanel::new(egui::panel::TopBottomSide::Top, "page_panel").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut app.current_page,
                InterfacePage::Downloader,
                label!("download", DOWNLOADER_ICON),
            );
            ui.selectable_value(
                &mut app.current_page,
                InterfacePage::History,
                label!("history", HISTORY_ICON),
            );
            ui.selectable_value(
                &mut app.current_page,
                InterfacePage::Settings,
                label!("settings", SETTINGS_ICON),
            );
        });
    });
}

pub fn downloader(app: &mut App, ui: &mut Ui) {
    ui.vertical_centered_justified(|ui| {
        spacer(ui);
        TextEdit::singleline(&mut app.downloader_state.song.source)
            .hint_text("enter query url...")
            .horizontal_align(egui::Align::Center)
            .show(ui);

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
                            Label::new(RichText::new("?").size(iconst!(COVER_SIZE) * 0.15))
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
                        spacer(ui);
                        spacer(ui);
                        ui.group(|ui| {
                            spacer(ui);
                            path_edit(ui, &mut app.downloader_state.save_path);
                            ui.add_enabled_ui(app.downloader_state.save_path.exists(), |ui| {
                                if ui.button("save").clicked() {
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

fn path_edit(ui: &mut Ui, path: &mut PathBuf) {
    // let tedit_resp = ui.add(
    // [ui.available_width(), iconst!(DETAILS_ROW_HEIGHT)],
    let tedit_response = TextEdit::singleline(&mut path.as_path().to_string_lossy().to_string())
        .hint_text("click to set path")
        .interactive(false)
        .show(ui)
        .response;
    // );

    if ui
        .interact(
            tedit_response.rect,
            tedit_response.id.with("click_sense"),
            Sense::click(),
        )
        .clicked()
    {
        if let Some(path_buf) = rfd::FileDialog::new().pick_folder() {
            *path = path_buf;
        }
    }
}

pub fn root(app: &mut App, ctx: &Context) {
    draw_side_panel(app, ctx);

    CentralPanel::default().show(ctx, |ui| match app.current_page {
        InterfacePage::Downloader => downloader(app, ui),
        _ => {
            ui.label("not done yet");
        }
    });
}

pub mod constants {
    use egui::{vec2, Vec2};

    pub const SPACER_SIZE: f32 = 5.;
    pub const DOWNLOADER_ICON: &str = "📥";
    pub const HISTORY_ICON: &str = egui_phosphor::BOOK_OPEN_TEXT;
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
