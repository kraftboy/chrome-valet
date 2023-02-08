// #![windows_subsystem = "windows"]

use std::os::windows::process::CommandExt;
use std::process::Command;
use std::sync::Arc;
use std::sync::Mutex;

use eframe::egui;
use egui::{TextureOptions, TextureFilter, ImageData, ColorImage};
use device_query::{DeviceQuery, DeviceState, Keycode};
use clap::Parser;

mod chrome_interface;
mod registry_utils;
use chrome_interface::ChromeInterface;

const DETACHED_PROCESS: u32 = 0x00000008;

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
   /// Create registry keys
   #[arg(long, default_value = "false")]
   create_keys: bool,

   /// Delete registry keys
   #[arg(long, default_value = "false")]
   delete_keys: bool,

   /// Url to open
   #[arg(long, default_value = "")]
   url: String,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() {

    let args = Args::parse();
    println!("args: {:?}", args);

    if args.create_keys {
        match registry_utils::create_registry_keys()
        {
            Err(e) => println!("Error creating registry keys: {:?}", e),
            _ => println!("Registry keys created."),
        }
        return;
    } else if args.delete_keys {
        match registry_utils::delete_registry_keys()
        {
            Err(e) => println!("Error deleting registry keys: {:?}", e),
            _ => println!("Registry keys deleted."),
        }
        return;
    } else {  
        
        let mut chrome = chrome_interface::ChromeInterface::new();
        if !chrome.populate_profile_entries()
        {
            // a "panic" should at least open a dang browser
            open_url_in_chrome(&args.url, &String::default());
        }

        let last_used_profile_name = match chrome.get_lastused_profile()
        {
            Some(x) => x,
            None => {
                "No profile found".to_string();
                return;
            },
        };
           
        if !chrome.profile_entries.iter().any(|x| { return x.profile_directory == last_used_profile_name; })
        {
            println!("last used profile {} not found in profile_names!", last_used_profile_name);
            return;
        }

        // todo: 
        // - surpress default browser warning (Chrome/Default/Preferences: check_default_browser)
        // - enumerate all (chromium?) browsers and handle openers programmatically?
    
        let ci_arcm = Arc::new(Mutex::new(chrome));

        let profile_picture_fetch = ci_arcm.clone();
        for entry in &mut profile_picture_fetch.lock().unwrap().profile_entries {
            let profile_picture_borrow = entry.profile_picture.clone();
            tokio::runtime::Handle::current().spawn(
                async move
                {
                    let mut locked_picture = profile_picture_borrow.lock().await;
                    let fetch_picture_result = locked_picture.get_picture().await;
                    if fetch_picture_result.is_err() {
                        println!("error fetching picture: {}", fetch_picture_result.err().unwrap());
                    }
                }
            );
        }
        
        /* 
        let device_state = DeviceState::new();
        let keys: Vec<Keycode> = device_state.get_keys();
        if !keys.contains(&Keycode::LControl)
        {
            open_url_in_chrome(&args.url, &last_used_profile_name);
            return;
        }
        */

        // actually run the app
        let options = eframe::NativeOptions {
            initial_window_size: Some(egui::vec2(50.0, 50.0)),
            resizable: false,
            ..Default::default()
        };

        eframe::run_native(
            "Chromefilla",
            options,
            Box::new(|_cc| Box::new(MyApp {chrome_interface: ci_arcm })),
        );
    }
}

struct MyApp {
    chrome_interface: Arc<Mutex<ChromeInterface>>,
}

impl MyApp
{
    const BUTTON_SIZE: f32 = 30.0;
}

impl eframe::App for MyApp {

    fn persist_native_window(&self) -> bool
    {
        return false;
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {

            ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                
                ui.with_layout(egui::Layout::top_down(egui::Align::TOP), |ui| {
                    for profile_entry in &self.chrome_interface.lock().unwrap().profile_entries {
                        let mut button = egui::Button::new(profile_entry.profile_directory.clone());
                        button = button.min_size(egui::vec2(200.0, MyApp::BUTTON_SIZE));
                        ui.add(button);
                    }
                });

                ui.with_layout(egui::Layout::top_down(egui::Align::TOP), |ui| {
                    for profile_entry in &self.chrome_interface.lock().unwrap().profile_entries {

                        let mut lock = profile_entry.profile_picture.try_lock();
                        if let Some(ref mut _mutex) = lock {
                            let mut profile_picture = lock.unwrap();
                            if profile_picture.img != None
                            {
                                let profile_image_copy = profile_picture.img.clone();
                                let texture: &egui::TextureHandle = profile_picture.profile_texture.get_or_insert_with(|| {
                                    // Load the texture only once.
                                    ui.ctx().load_texture(
                                        format!("{} Profile Pic Texture", profile_entry.profile_name),
                                        profile_image_copy.unwrap(),
                                        Default::default()
                                    )
                                });
            
                                ui.image(texture, egui::vec2(MyApp::BUTTON_SIZE, MyApp::BUTTON_SIZE));
                            }
                            else {
                                let texture: &egui::TextureHandle = profile_picture.profile_texture.get_or_insert_with(|| {
                                    // Load the texture only once.
                                    ui.ctx().load_texture(
                                        format!("{} Profile Pic Texture", profile_entry.profile_name),
                                        ColorImage::from_rgba_unmultiplied([1, 1], &[1, 0, 0, 1]),
                                        Default::default()
                                    )
                                });
                            }
                        };
                    }
                });
            });
        });
    }
}

fn open_url_in_chrome(url: &String, profile_name: &String)
{
    println!("Open url: {}", url);
    let mut chrome_command_line: String = String::default();
    match registry_utils::get_chrome_exe()
    {
        Err(e) => println!("Failed to get chrome open command: {}", e),
        Ok(v) => chrome_command_line = v,
    }

    let mut chrome_command = Command::new(chrome_command_line);
    chrome_command.creation_flags(DETACHED_PROCESS);

    if profile_name.is_empty() {
        chrome_command.arg(format!("--profile-directory={}", profile_name));
    }
    
    chrome_command.arg("--single-argument").arg(url);

    let chrome_command_child_result = chrome_command.spawn();

    println!("command: {:?}", chrome_command);

    match chrome_command_child_result
    {
        Err(e) => println!("Error excecuting command: {}", e),
        _ => (),
    };
}
