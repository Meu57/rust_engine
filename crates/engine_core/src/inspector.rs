use egui::{Color32, Context, Ui};
use engine_shared::{PriorityLayer, ActionSignal, MovementSignal};
use crate::input::Arbiter;

pub fn show(ctx: &Context, arbiter: &Arbiter, open: &mut bool) {
    egui::Window::new("Input Inspector")
        .open(open)
        .show(ctx, |ui| {
            ui.heading("Arbiter State");
            ui.separator();

            // 1. ANALYZE MOVEMENT (Still Winner-Takes-All)
            let mut move_winner = PriorityLayer::Ambient;
            for &layer in &[PriorityLayer::Reflex, PriorityLayer::Cutscene, PriorityLayer::Control] {
                if arbiter.move_signals.iter().any(|s| s.layer == layer) {
                    move_winner = layer;
                    break;
                }
            }

            ui.label(format!("Movement Winner: {:?}", move_winner));
            
            // Visualize Movement Signals
            ui.collapsing("Movement Signals", |ui| {
                for signal in &arbiter.move_signals {
                    let is_winner = signal.layer == move_winner;
                    let color = if is_winner { Color32::GREEN } else { Color32::GRAY };
                    
                    ui.colored_label(color, format!(
                        "[{:?}] {:?} (w: {:.1}) -> {}", 
                        signal.layer, 
                        signal.vector, 
                        signal.weight,
                        if is_winner { "ACTIVE" } else { "SUPPRESSED" }
                    ));
                }
            });

            ui.separator();

            // 2. ANALYZE ACTIONS (Now Cumulative!)
            ui.label("Action Contributors:");

            // We iterate layers to see which ones are actually contributing to the mask
            let layers = [PriorityLayer::Reflex, PriorityLayer::Cutscene, PriorityLayer::Control];
            for layer in layers {
                // Check if this layer has any active signals
                let has_active = arbiter.action_signals.iter().any(|s| s.layer == layer && s.active);
                
                if has_active {
                    // Draw it in Green to show it is contributing
                    ui.colored_label(Color32::GREEN, format!("  • {:?}", layer));
                } else {
                    // Draw faint text to show it's idle
                    ui.colored_label(Color32::from_gray(100), format!("  • {:?} (Idle)", layer));
                }
            }

            ui.collapsing("Action Signals (Raw)", |ui| {
                for signal in &arbiter.action_signals {
                    // In the new system, ANY active signal from a valid layer contributes.
                    // So if it's active, it's green.
                    let color = if signal.active { Color32::GREEN } else { Color32::RED };
                    
                    ui.colored_label(color, format!(
                        "[{:?}] ID: {} = {}", 
                        signal.layer, 
                        signal.action_id, 
                        signal.active
                    ));
                }
            });
        });
}