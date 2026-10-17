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

use window::{WispUserEvent, WispWindow};

fn main() {
    env_logger::init();

    let event_loop = EventLoop::<WispUserEvent>::with_user_event()
        .build()
        .expect("Failed to create event loop");
    let mut wisp_window = WispWindow::new(&event_loop);

    // Sleep until the next deadline (blink / sync failsafe) or an event;
    // the PTY reader wakes us via a user event. Never busy-poll.
    event_loop.set_control_flow(ControlFlow::WaitUntil(wisp_window.next_deadline()));
    wisp_window.window().request_redraw();

    let mut modifiers = ModifiersState::empty();
    let mut pressed_buttons: Vec<MouseButton> = Vec::new();

    event_loop
        .run(move |event, elwt| {
            match event {
                Event::UserEvent(WispUserEvent::PtyOutput) => {
                    if wisp_window.process_pty() && !wisp_window.sync_holding() {
                        wisp_window.window().request_redraw();
                    }
                }
                Event::WindowEvent { event, window_id }
                    if window_id == wisp_window.window().id() =>
                {
                    match event {
                        WindowEvent::CloseRequested => {
                            log::debug!("Close requested, exiting...");
                            elwt.exit();
                        }
                        WindowEvent::Resized(physical_size) => {
                            wisp_window.resize(physical_size);
                        }
                        WindowEvent::RedrawRequested => {
                            // Drain any bytes queued since the redraw was
                            // requested, then present unless a sync hold
                            // is still active.
                            wisp_window.process_pty();
                            if !wisp_window.sync_holding() {
                                wisp_window.render();
                            }
                        }
                        WindowEvent::ModifiersChanged(new_modifiers) => {
                            modifiers = new_modifiers.state();
                        }
                        WindowEvent::KeyboardInput { event, .. } => {
                            if event.state == ElementState::Pressed {
                                // Copy: Cmd+C (macOS) or Ctrl+Shift+C (others)
                                let is_c =
                                    event.physical_key == PhysicalKey::Code(KeyCode::KeyC);
                                let copy_combo = modifiers.super_key()
                                    || (modifiers.control_key() && modifiers.shift_key());
                                // Paste: Cmd+V (macOS) or Ctrl+Shift+V (others).
                                let is_v =
                                    event.physical_key == PhysicalKey::Code(KeyCode::KeyV);
                                let paste_combo = modifiers.super_key()
                                    || (modifiers.control_key() && modifiers.shift_key());

                                if is_c && copy_combo {
                                    wisp_window.copy_selection();
                                } else if is_v && paste_combo {
                                    wisp_window.paste();
                                } else {
                                    let text =
                                        event.text.as_ref().map(|s| s.as_str()).unwrap_or("");
                                    log::debug!(
                                        "Key pressed: physical_key={:?}, text='{}', logical_key={:?}",
                                        event.physical_key,
                                        text,
                                        event.logical_key
                                    );
                                    if let Some(bytes) = input::key_to_bytes(
                                        event.physical_key,
                                        text,
                                        &event.logical_key,
                                        modifiers,
                                    ) {
                                        log::debug!("Sending bytes: {:?}", bytes);
                                        wisp_window.send_input(&bytes);
                                    }
                                }
                            }
                        }
                        WindowEvent::Focused(focused) => {
                            wisp_window.focus_changed(focused);
                        }
                        WindowEvent::MouseInput { state, button, .. } => match state {
                            ElementState::Pressed => {
                                if !pressed_buttons.contains(&button) {
                                    pressed_buttons.push(button);
                                }
                                wisp_window.mouse_press(button, modifiers);
                            }
                            ElementState::Released => {
                                pressed_buttons.retain(|b| *b != button);
                                wisp_window.mouse_release(button, modifiers);
                            }
                        },
                        WindowEvent::CursorMoved { position, .. } => {
                            wisp_window.mouse_move(position, modifiers, &pressed_buttons);
                        }
                        WindowEvent::MouseWheel { delta, .. } => {
                            let delta_y = match delta {
                                MouseScrollDelta::LineDelta(_x, y) => y * 3.0,
                                MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                            };
                            wisp_window.mouse_wheel(delta_y, modifiers);
                        }
                        WindowEvent::Ime(ime) => match ime {
                            Ime::Commit(text) => {
                                log::debug!("IME Commit: '{}'", text);
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
                    // Close the window once the shell has exited (009);
                    // bounded latency via the blink/sync deadline below.
                    if wisp_window.shell_exited() {
                        elwt.exit();
                    }
                    // Deadline reached (or events drained): process any
                    // queued output, advance blink/sync state, and redraw
                    // only on a real change.
                    let processed = wisp_window.process_pty();
                    let ticked = wisp_window.tick();
                    if (processed || ticked) && !wisp_window.sync_holding() {
                        wisp_window.window().request_redraw();
                    }
                }
                _ => {}
            }

            elwt.set_control_flow(ControlFlow::WaitUntil(wisp_window.next_deadline()));
        })
        .expect("Event loop error");
}
