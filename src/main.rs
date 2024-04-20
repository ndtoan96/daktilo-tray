use std::sync::mpsc;

use daktilo_lib::{app::App, audio, embed::EmbeddedConfig};
use rdev::listen;
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::{
    menu::{CheckMenuItem, CheckMenuItemBuilder, Menu, MenuEvent, MenuId, MenuItem, Submenu},
    ClickType, TrayIconBuilder, TrayIconEvent,
};

const ICON_ENABLED: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/typewritter_icon_enabled.png"
));
const ICON_DISABLED: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/typewritter_icon_disabled.png"
));

fn main() {
    let presets = EmbeddedConfig::parse().unwrap().sound_presets;
    let devices = audio::get_devices().expect("Fail to get computer audio devices");

    // Spawn a thread to listen to key events
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        listen(move |event| {
            tx.send(event)
                .unwrap_or_else(|e| tracing::error!("could not send event {:?}", e));
        })
        .expect("could not listen events");
    });

    // Spawn a thread to play sound
    let presets_clone = presets.clone();
    std::thread::spawn(move || {
        let preset = presets_clone.first().unwrap().clone();
        let mut app = App::init(preset, None, None).unwrap();
        loop {
            if let Ok(event) = rx.recv() {
                app.handle_key_event(event.clone()).unwrap();
            }
        }
    });

    let enabled_icon = load_icon(ICON_ENABLED);
    let disabled_icon = load_icon(ICON_DISABLED);
    let presets_menu = Submenu::new("presets", true);
    let devices_menu = Submenu::new("devices", true);
    let enable_menu = MenuItem::new("disable", true, None);
    let exit_menu = MenuItem::with_id(MenuId("exit".to_string()), "exit", true, None);
    let preset_items: Vec<_> = presets
        .iter()
        .enumerate()
        .map(|(i, p)| {
            CheckMenuItemBuilder::new()
                .id(MenuId(format!("preset_{i}")))
                .text(&p.name)
                .enabled(true)
                .checked(p.name == "default")
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
                .checked(i == 0)
                .build()
        })
        .collect();
    for item in device_items.iter() {
        devices_menu.append(item).unwrap();
    }
    let mut tray_icon = None;

    let menu_channel = MenuEvent::receiver();
    let event_loop = EventLoopBuilder::new().build();
    let mut enabled = true;
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
                    .with_icon(enabled_icon.clone())
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
                if enabled {
                    enabled = false;
                    enable_menu.set_text("enable");
                    tray_icon
                        .as_mut()
                        .unwrap()
                        .set_icon(Some(disabled_icon.clone()))
                        .unwrap();
                } else {
                    enabled = true;
                    enable_menu.set_text("disable");
                    tray_icon
                        .as_mut()
                        .unwrap()
                        .set_icon(Some(enabled_icon.clone()))
                        .unwrap();
                }
            }
            // Exit app
            else if event.id() == exit_menu.id() {
                *control_flow = ControlFlow::ExitWithCode(0);
            } else {
                let MenuId(id) = event.id();
                // Change preset
                if id.starts_with("preset_") {
                    let checked_i: usize = (&id.strip_prefix("preset_").unwrap()).parse().unwrap();
                    preset_items
                        .iter()
                        .enumerate()
                        .for_each(|(i, p)| p.set_checked(i == checked_i));
                }
                // Change audio device
                else if id.starts_with("device_") {
                    let checked_i: usize = (&id.strip_prefix("device_").unwrap()).parse().unwrap();
                    device_items
                        .iter()
                        .enumerate()
                        .for_each(|(i, p)| p.set_checked(i == checked_i));
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
