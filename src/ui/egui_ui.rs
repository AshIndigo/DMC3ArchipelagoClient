// Nothing wrong with this. Runs as a separate window so would have to alt-tab if full screen. Could at least be a backup HUD if DDMK isn't available

use crate::constants::{ItemCategory, Status};
use crate::ui::ui;
use crate::ui::ui::{CHECKLIST, CONNECTION_STATUS};
use crate::{bank, constants};
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
        });
    }
}

fn setup_bank_grid(ui: &mut Ui) {
    let bank = bank::get_bank().read().unwrap();
    let mut id = 50;
    egui::Grid::new(id).num_columns(3).spacing([4.0, 4.0]).show(ui, |ui| {
        for item in constants::get_items_by_category(ItemCategory::Consumable) {
                ui.label(format!("{}:", item));
                ui.label(bank.get(item).unwrap().to_string());
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
            // ui.label(format!("Blue Orbs: {}", BLUE_ORBS_OBTAINED.load(Ordering::Relaxed)));
            // ui.label(format!("Purple Orbs: {}", PURPLE_ORBS_OBTAINED.load(Ordering::Relaxed)));
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
