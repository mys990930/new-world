use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::WindowId;

use super::GameApp;

impl ApplicationHandler for GameApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.platform.resumed(event_loop);

        if let Some(window) = self.platform.window_handle() {
            if let Err(error) = self.renderer.attach_window_surface(window) {
                eprintln!("[app] renderer surface attach failed: {:?}", error);
            }
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.platform.suspended();
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, _cause: StartCause) {}

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if !self.platform.handle_window_event(window_id, &event) {
            return;
        }

        if self.platform.window_state().close_requested
            || self.platform.lifecycle_state().quit_requested
        {
            event_loop.exit();
        }

        if let WindowEvent::RedrawRequested = event {
            self.render();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.platform.window_state().close_requested
            || self.platform.lifecycle_state().quit_requested
        {
            event_loop.exit();
            return;
        }

        let now = Instant::now();
        if !self.should_run_frame(now) {
            if let Some(deadline) = self.frame_deadline() {
                event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
            } else {
                event_loop.set_control_flow(ControlFlow::Poll);
            }
            return;
        }

        self.begin_timed_frame(now);
        self.update();
        self.platform.request_redraw();
        self.platform.end_frame();
        self.platform.begin_frame();

        if let Some(deadline) = self.frame_deadline() {
            event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
        } else {
            event_loop.set_control_flow(ControlFlow::Poll);
        }
    }
}
