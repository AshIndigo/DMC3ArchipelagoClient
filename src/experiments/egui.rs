// Nothing wrong with this. Runs as a separate window so would have to alt-tab if full screen. Could at least be a backup HUD if DDMK isn't available

use eframe::{EventLoopBuilderHook, Frame};
use egui::{Context, Theme};
use winit::platform::windows::EventLoopBuilderExtWindows;

#[derive(Default)]
struct ArchipelagoClient;

impl eframe::App for ArchipelagoClient {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if ui.button("This produces Debug Info").clicked() {
                log::debug!("Very verbose Debug Info")
            }
            if ui.button("This produces an Info").clicked() {
                log::info!("Some Info");
            }
            if ui.button("This produces an Error").clicked() {
                log::error!("Error doing Something");
            }
            if ui.button("This produces a Warning").clicked() {
                log::warn!("Warn about something")
            }
        });
        egui::Window::new("Log").show(ctx, |ui| {
            // draws the actual logger ui
            egui_logger::LoggerUi::default().enable_regex(true).show(ui);
        });
    }
}

pub fn start_egui() {
    let event_loop_builder: Option<EventLoopBuilderHook> = Some(Box::new(|event_loop_builder| {
        event_loop_builder.with_any_thread(true);
    }));
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
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
