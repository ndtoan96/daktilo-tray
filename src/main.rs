// #![windows_subsystem = "windows"]

use daktilo_lib::{app::App, audio, embed::EmbeddedConfig};
use rdev::listen;
use rodio::{cpal::traits::HostTrait, DeviceTrait};
use serde::{Deserialize, Serialize};
use std::sync::mpsc;
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tracing_subscriber::prelude::*;
use tray_icon::{
    menu::{CheckMenuItemBuilder, Menu, MenuEvent, MenuId, MenuItem, Submenu},
    TrayIconBuilder,
};

const ICON_ENABLED: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/typewritter_icon_enabled.png"
));
const ICON_DISABLED: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/typewritter_icon_disabled.png"
));

enum EventKind {
    KeyEvent(rdev::Event),
    ChangeConfig {
        preset_name: String,
        device_name: String,
    },
    Enabled(bool),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct State {
    enabled: bool,
    current_preset_name: String,
    current_device_name: String,
}

fn main() {
    // Set up tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let presets = EmbeddedConfig::parse().unwrap().sound_presets;
    let devices = audio::get_devices().expect("Fail to get computer audio devices");
    let (tx, rx) = mpsc::channel();

    // App states
    let cache_path = directories::BaseDirs::new()
        .unwrap()
        .cache_dir()
        .join("daktilo_tray_cache.toml");
    let mut state = if let Ok(content) = std::fs::read_to_string(&cache_path) {
        let mut cached_state: State = toml::from_str(&content).unwrap();
        if !rodio::cpal::default_host()
            .output_devices()
            .unwrap()
            .any(|d| {
                d.name().unwrap_or_default().to_lowercase() == cached_state.current_device_name
            })
        {
            cached_state.current_device_name = rodio::cpal::default_host()
                .default_output_device()
                .unwrap()
                .name()
                .unwrap()
                .to_lowercase();
        }
        cached_state
    } else {
        State {
            enabled: true,
            current_preset_name: String::from("default"),
            current_device_name: rodio::cpal::default_host()
                .default_output_device()
                .unwrap()
                .name()
                .unwrap()
                .to_lowercase(), // for whatever reason, the App::init check agains lowercase device name
        }
    };
    tracing::debug!("{:?}", &state);

    // Spawn a thread to listen to key events
    let tx1 = tx.clone();
    std::thread::spawn(move || {
        listen(move |event| {
            tx1.send(EventKind::KeyEvent(event))
                .unwrap_or_else(|e| tracing::error!("could not send event {:?}", e));
        })
        .expect("could not listen events");
    });

    // Spawn a thread to play sound
    let presets_clone = presets.clone();
    let init_device_name = state.current_device_name.clone();
    let init_preset_name = state.current_preset_name.clone();
    let mut enabled = state.enabled;
    tracing::debug!("Current device: {}", state.current_device_name);
    std::thread::spawn(move || {
        let preset = presets_clone
            .iter()
            .find(|p| p.name == init_preset_name)
            .unwrap();
        let mut app = App::init(preset.clone(), None, Some(init_device_name)).unwrap();
        loop {
            match rx.recv() {
                Ok(EventKind::KeyEvent(event)) => {
                    if enabled {
                        app.handle_key_event(event.clone()).unwrap()
                    }
                }
                Ok(EventKind::ChangeConfig {
                    preset_name,
                    device_name,
                }) => {
                    let preset = presets_clone
                        .iter()
                        .find(|p| p.name == preset_name)
                        .unwrap();
                    app =
                        App::init(preset.clone(), None, Some(device_name.to_lowercase())).unwrap();
                }
                Ok(EventKind::Enabled(is_enabled)) => enabled = is_enabled,
                Err(e) => {
                    tracing::error!("{}", e);
                }
            }
        }
    });

    let enabled_icon = load_icon(ICON_ENABLED);
    let disabled_icon = load_icon(ICON_DISABLED);
    let presets_menu = Submenu::new("presets", true);
    let devices_menu = Submenu::new("devices", true);
    let enable_menu = MenuItem::new(if state.enabled { "disable" } else { "enable" }, true, None);
    let exit_menu = MenuItem::with_id(MenuId("exit".to_string()), "exit", true, None);
    let preset_items: Vec<_> = presets
        .iter()
        .enumerate()
        .map(|(i, p)| {
            CheckMenuItemBuilder::new()
                .id(MenuId(format!("preset_{i}")))
                .text(&p.name)
                .enabled(true)
                .checked(p.name == state.current_preset_name)
                .build()
        })
        .collect();
    for item in preset_items.iter() {
        presets_menu.append(item).unwrap();
    }
    let device_items: Vec<_> = devices
        .iter()
        .enumerate()
        .map(|(i, (name, _))| {
            CheckMenuItemBuilder::new()
                .id(MenuId(format!("device_{i}")))
                .text(name)
                .enabled(true)
                .checked(name.to_lowercase() == state.current_device_name)
                .build()
        })
        .collect();
    for item in device_items.iter() {
        devices_menu.append(item).unwrap();
    }
    let mut tray_icon = None;

    let menu_channel = MenuEvent::receiver();
    let event_loop = EventLoopBuilder::new().build();
    let tx2 = tx.clone();
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let tao::event::Event::NewEvents(tao::event::StartCause::Init) = event {
            // We create the icon once the event loop is actually running
            // to prevent issues like https://github.com/tauri-apps/tray-icon/issues/90
            // Creating tray icon
            let tray_menu = Menu::new();
            tray_menu
                .append_items(&[&presets_menu, &devices_menu, &enable_menu, &exit_menu])
                .unwrap();
            tray_icon = Some(
                TrayIconBuilder::new()
                    .with_menu(Box::new(tray_menu))
                    .with_icon(if state.enabled {
                        enabled_icon.clone()
                    } else {
                        disabled_icon.clone()
                    })
                    .with_tooltip("Daktilo Tray")
                    .build()
                    .unwrap(),
            );

            // We have to request a redraw here to have the icon actually show up.
            // Tao only exposes a redraw method on the Window so we use core-foundation directly.
            #[cfg(target_os = "macos")]
            unsafe {
                use core_foundation::runloop::{CFRunLoopGetMain, CFRunLoopWakeUp};

                let rl = CFRunLoopGetMain();
                CFRunLoopWakeUp(rl);
            }
        }

        if let Ok(event) = menu_channel.try_recv() {
            // Enable/disable app
            if event.id() == enable_menu.id() {
                if state.enabled {
                    state.enabled = false;
                    enable_menu.set_text("enable");
                    tray_icon
                        .as_mut()
                        .unwrap()
                        .set_icon(Some(disabled_icon.clone()))
                        .unwrap();
                } else {
                    state.enabled = true;
                    enable_menu.set_text("disable");
                    tray_icon
                        .as_mut()
                        .unwrap()
                        .set_icon(Some(enabled_icon.clone()))
                        .unwrap();
                }
                tx2.send(EventKind::Enabled(state.enabled)).unwrap();
            }
            // Exit app
            else if event.id() == exit_menu.id() {
                std::fs::write(&cache_path, toml::to_string(&state).unwrap()).unwrap();
                *control_flow = ControlFlow::ExitWithCode(0);
            } else {
                let MenuId(id) = event.id();
                // Change preset
                if id.starts_with("preset_") {
                    let checked_i: usize = (id.strip_prefix("preset_").unwrap()).parse().unwrap();
                    preset_items.iter().enumerate().for_each(|(i, p)| {
                        if i == checked_i {
                            state.current_preset_name = p.text();
                            tx2.send(EventKind::ChangeConfig {
                                preset_name: state.current_preset_name.clone(),
                                device_name: state.current_device_name.clone(),
                            })
                            .unwrap();
                        }
                        p.set_checked(i == checked_i);
                    });
                }
                // Change audio device
                else if id.starts_with("device_") {
                    let checked_i: usize = (id.strip_prefix("device_").unwrap()).parse().unwrap();
                    device_items.iter().enumerate().for_each(|(i, d)| {
                        if i == checked_i {
                            state.current_device_name = d.text().to_lowercase();
                            tx2.send(EventKind::ChangeConfig {
                                preset_name: state.current_preset_name.clone(),
                                device_name: state.current_device_name.clone(),
                            })
                            .unwrap();
                        }
                        d.set_checked(i == checked_i)
                    });
                } else {
                    unreachable!();
                }
            }
            println!("{event:?}");
        }
    });
}

fn load_icon(bytes: &[u8]) -> tray_icon::Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::load_from_memory(bytes)
            .expect("Failed to open icon path")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    tray_icon::Icon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to open icon")
}
