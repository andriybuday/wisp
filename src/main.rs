use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
};

mod window;
mod renderer;
mod terminal;
mod config;

use window::WispWindow;

fn main() {
    env_logger::init();
    
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut wisp_window = WispWindow::new(&event_loop);
    
    event_loop.set_control_flow(ControlFlow::Wait);
    
    event_loop.run(move |event, elwt| {
        match event {
            Event::WindowEvent { event, window_id } 
                if window_id == wisp_window.window().id() => {
                match event {
                    WindowEvent::CloseRequested => {
                        println!("Close requested, exiting...");
                        elwt.exit();
                    }
                    WindowEvent::Resized(physical_size) => {
                        wisp_window.resize(physical_size);
                    }
                    WindowEvent::RedrawRequested => {
                        wisp_window.render();
                    }
                    _ => {}
                }
            }
            Event::AboutToWait => {
                wisp_window.window().request_redraw();
            }
            _ => {}
        }
    }).expect("Event loop error");
}
