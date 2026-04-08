pub fn main() {
    run();
}

pub fn run() {
    let mut app = App::new();
    app.run();
}

pub struct App {
    running: bool,
}

impl App {
    pub fn new() -> Self {
        Self { running: true }
    }

    pub fn run(&mut self) {
        while self.running {
            self.begin_frame();
            self.poll_platform();
            self.end_frame();
        }
    }

    fn begin_frame(&mut self) {
        // frame start
    }

    fn poll_platform(&mut self) {
        // platform input/window polling placeholder
    }

    fn end_frame(&mut self) {
        // frame end
    }
}
