// Nothing wrong with this. Runs as a separate window so would have to alt-tab if full screen. Could at least be a backup HUD if DDMK isn't available

use crate::archipelago::CHECKLIST;
use crate::constants::{ItemCategory, Status};
use crate::hook::CONNECTION_STATUS;
use crate::ui::ui;
use crate::{archipelago, bank, constants};
use eframe::epaint::Color32;
use eframe::{EventLoopBuilderHook, Frame};
use egui::{Context, Theme, Ui};
use std::sync::atomic::Ordering;
use winit::platform::windows::EventLoopBuilderExtWindows;

#[derive(Default)]
struct ArchipelagoClient;

impl eframe::App for ArchipelagoClient {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        egui::TopBottomPanel::bottom("log_panel").show(ctx, |ui| {
            egui_logger::LoggerUi::default().enable_regex(true).show(ui);
        });
        egui::SidePanel::right("tracker_bank").show(ctx, |ui| {
            ui.heading("Key items");
            setup_tracker_grid(ui);
            ui.separator();
            ui.heading("Bank");
            setup_bank_grid(ui);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Connection");
            ui.horizontal(|ui| {
                ui.label("Status:");
                ui.colored_label(get_status_color(), ui::get_status_text());
            });
            match archipelago::get_hud_data().lock() {
                Ok(mut instance) => {
                    ui.horizontal(|ui| {
                        ui.label("URL:");
                        ui.text_edit_singleline(&mut instance.archipelago_url);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Username:");
                        ui.text_edit_singleline(&mut instance.username);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Password:");
                        ui.text_edit_singleline(&mut instance.password);
                    });
                    if ui.button("Connect").clicked() {
                        log::debug!("Connecting");
                        ui::connect_button_pressed(
                            instance.archipelago_url.clone(),
                            instance.username.clone(),
                            instance.password.clone(),
                        );
                    }
                }
                Err(err) => {
                    log::error!("Unable to lock HUD Data: {}", err);
                }
            }
        });

        /*        egui::Window::new("Log").show(ctx, |ui| {
            // draws the actual logger ui
            egui_logger::LoggerUi::default().enable_regex(true).show(ui);
        });*/
    }
}

fn setup_bank_grid(ui: &mut Ui) {
    let bank = bank::get_bank().lock().unwrap();
    let mut id = 50;
    egui::Grid::new(id).spacing([25.0, 4.0]).show(ui, |ui| {
        for item in constants::get_items_by_category(ItemCategory::Consumable) {
            ui.horizontal(|ui| {
                ui.label(format!("{}:", item));
                ui.label(bank.get(item).unwrap().to_string());
                if ui.button("Retrieve 1").clicked() {
                    if bank::can_add_item_to_current_inv(item) {
                        ui::retrieve_button_pressed(item);
                    }
                };
            });
            id += 1;
            ui.end_row();
        }
    });
}

fn get_status_color() -> Color32 {
    match CONNECTION_STATUS.load(Ordering::Relaxed).into() {
        Status::Connected => Color32::GREEN,
        Status::Disconnected => Color32::GRAY,
        _ => Color32::RED,
    }
}

fn setup_tracker_grid(ui: &mut Ui) {
    let checklist = CHECKLIST.get().and_then(|lock| lock.read().ok()).unwrap();
    let mut id = 0;
    egui::Grid::new(id)
        .striped(true)
        .spacing([10.0, 4.0])
        .show(ui, |ui| {
            for row in constants::get_items_by_category(ItemCategory::Key).chunks(3) {
                for item in row {
                    ui.label(format!("{}", item));
                    ui.label(if *checklist.get(*item).unwrap_or(&false) {
                        "☑"
                    } else {
                        "☐"
                    });
                }
                id += 1;
                ui.end_row();
            }
        });
}

pub fn start_egui() {
    let event_loop_builder: Option<EventLoopBuilderHook> = Some(Box::new(|event_loop_builder| {
        event_loop_builder.with_any_thread(true);
    }));
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_min_inner_size([860.0, 480.0]),
        event_loop_builder,
        ..Default::default()
    };

    eframe::run_native(
        "Archipelago Client",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_theme(Theme::Dark);
            Ok(Box::new(ArchipelagoClient))
        }),
    )
    .unwrap();
}
