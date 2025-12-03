// crates/engine_core/src/inspector.rs
use egui;
use engine_shared::PriorityLayer;
use crate::input::Arbiter;

pub fn show(ctx: &egui::Context, arbiter: &Arbiter, open: &mut bool) {
    if !*open {
        return;
    }

    egui::Window::new("Input Inspector")
        .default_pos([10.0, 10.0])
        .show(ctx, |ui| {
            ui.heading("Arbitration Stack");
            ui.separator();

            let layers = [
                (PriorityLayer::Reflex, "Layer 0: Reflex (Physics)", egui::Color32::RED),
                (PriorityLayer::Cutscene, "Layer 1: Cutscene", egui::Color32::YELLOW),
                (PriorityLayer::Control, "Layer 2: Player Control", egui::Color32::GREEN),
                (PriorityLayer::Ambient, "Layer 3: Ambient", egui::Color32::GRAY),
            ];

            let mut winning_move = PriorityLayer::Ambient;
            for &(layer, _, _) in &layers {
                if arbiter.move_signals.iter().any(|s| s.layer == layer) {
                    winning_move = layer;
                    break;
                }
            }

            for (layer, label, color) in layers {
                let is_active = arbiter.move_signals.iter().any(|s| s.layer == layer);
                let is_winner = layer == winning_move && is_active;

                if is_winner {
                    ui.colored_label(color, format!("▶ {} [WINNER]", label));
                    for s in arbiter.move_signals.iter().filter(|s| s.layer == layer) {
                        ui.label(format!("   Vector: {:.2}, Weight: {:.2}", s.vector, s.weight));
                    }
                } else if is_active {
                    ui.colored_label(color.linear_multiply(0.5), format!("▷ {} [SUPPRESSED]", label));
                } else {
                    ui.label(format!("  {}", label));
                }
            }

            ui.separator();
            ui.label("Press 'P' to simulate Reflex Layer override.");
            ui.colored_label(egui::Color32::LIGHT_GRAY, "Press 'F1' to hide this window.");
        });
}