use winit::{
    event::{ElementState, Event, Ime, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, ModifiersState, PhysicalKey},
};

mod config;
mod font;
mod input;
mod parser;
mod pty;
mod renderer;
mod terminal;
mod window;

use window::WispWindow;

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut wisp_window = WispWindow::new(&event_loop);

    event_loop.set_control_flow(ControlFlow::Poll);

    let mut modifiers = ModifiersState::empty();

    event_loop
        .run(move |event, elwt| match event {
            Event::WindowEvent { event, window_id } if window_id == wisp_window.window().id() => {
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
                    WindowEvent::ModifiersChanged(new_modifiers) => {
                        modifiers = new_modifiers.state();
                    }
                    WindowEvent::KeyboardInput { event, .. } => {
                        if event.state == ElementState::Pressed {
                            // Copy: Cmd+C (macOS) or Ctrl+Shift+C (others)
                            let is_c = event.physical_key == PhysicalKey::Code(KeyCode::KeyC);
                            let copy_combo = modifiers.super_key()
                                || (modifiers.control_key() && modifiers.shift_key());
                            if is_c && copy_combo {
                                wisp_window.copy_selection();
                                return;
                            }

                            // Paste: Cmd+V (macOS) or Ctrl+Shift+V (others).
                            let is_v = event.physical_key == PhysicalKey::Code(KeyCode::KeyV);
                            let paste_combo = modifiers.super_key()
                                || (modifiers.control_key() && modifiers.shift_key());
                            if is_v && paste_combo {
                                wisp_window.paste();
                                return;
                            }

                            let text = event.text.as_ref().map(|s| s.as_str()).unwrap_or("");
                            println!(
                                "Key pressed: physical_key={:?}, text='{}', logical_key={:?}",
                                event.physical_key, text, event.logical_key
                            );
                            if let Some(bytes) =
                                input::key_to_bytes(event.physical_key, text, &event.logical_key)
                            {
                                println!("Sending bytes: {:?}", bytes);
                                wisp_window.send_input(&bytes);
                            }
                        }
                    }
                    WindowEvent::MouseInput { state, button, .. } => {
                        if button == MouseButton::Left {
                            match state {
                                ElementState::Pressed => {
                                    wisp_window.mouse_press();
                                }
                                ElementState::Released => {
                                    wisp_window.mouse_release();
                                }
                            }
                        }
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        wisp_window.mouse_move(position);
                    }
                    WindowEvent::MouseWheel { delta, .. } => {
                        let delta_y = match delta {
                            MouseScrollDelta::LineDelta(_x, y) => y * 3.0,
                            MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                        };
                        wisp_window.mouse_wheel(delta_y);
                    }
                    WindowEvent::Ime(ime) => match ime {
                        Ime::Commit(text) => {
                            println!("IME Commit: '{}'", text);
                            wisp_window.send_input(text.as_bytes());
                        }
                        Ime::Preedit(_, _) => {
                            // Ignore preedit for now
                        }
                        Ime::Enabled | Ime::Disabled => {}
                    },
                    _ => {}
                }
            }
            Event::AboutToWait => {
                wisp_window.window().request_redraw();
            }
            _ => {}
        })
        .expect("Event loop error");
}
