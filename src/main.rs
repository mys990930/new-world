mod app;
mod ecs;
mod platform;
mod renderer;
mod world;

use std::error::Error;

use app::{AppConfig, GameApp};
use winit::event_loop::EventLoop;

fn main() -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new()?;
    let mut app = GameApp::new(AppConfig::default());
    event_loop.run_app(&mut app)?;
    Ok(())
}
