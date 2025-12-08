use std::env;
use engine_core::App;

fn main() {
    
    let args: Vec<String> = env::args().collect();
    let default_path = "target/debug/game_plugin.dll";
    let plugin_path = args.get(1).map(|s| s.as_str()).unwrap_or(default_path);

    App::new(plugin_path).run();
}