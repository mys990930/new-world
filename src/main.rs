use std::error::Error;

use new_world::app::{AppConfig, GameApp};
use winit::event_loop::EventLoop;

fn main() -> Result<(), Box<dyn Error>> {
    println!("[main] starting new-world");
    let event_loop = EventLoop::new()?;
    let mut app = GameApp::new(AppConfig::default());
    let result = event_loop.run_app(&mut app);
    println!("[main] event loop returned: {:?}", result);
    result?;
    Ok(())
}
