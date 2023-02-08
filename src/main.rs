// #![windows_subsystem = "windows"]

use std::os::windows::process::CommandExt;
use std::process::Command;
use std::process::exit;
use std::sync::Arc;
use std::sync::Mutex;
use std::str::FromStr;
use std::panic;
use std::str;
use log::LevelFilter;
use log::{debug,warn,info,error};

use eframe::egui;
use device_query::{DeviceQuery, DeviceState, Keycode};
use clap::Parser;

mod chrome_interface;
mod registry_utils;
use chrome_interface::ChromeInterface;

const DETACHED_PROCESS: u32 = 0x00000008;

type RegUtilityFn = fn() -> Result<(), registry::Error>;
fn do_utility_and_exit(util_fn: RegUtilityFn)
{
    match util_fn()
    {
        Err(e) => error!("Error creating registry keys: {:?}", e),
        _ => info!("Registry keys created."),
    }

    exit(0);
}

fn soft_panic(url: &String)
{
    open_url_in_chrome_and_exit(&url, &String::default());
}

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

   /// Log level (error, warn, info, debug, trace)
   #[arg(long, default_value = "info")]
   log_level: String,

    /// force ui to open
    #[arg(long, default_value = "false")]
    force_ui: bool,
}

static mut PANIC_URL: [u8;2048] = [0; 2048];

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() {

    let args = Args::parse();
    debug!("args: {:?}", args);

    // register minimum nice behaviour for panics, just open the damn browser
    unsafe { PANIC_URL[0..args.url.len()].copy_from_slice(args.url.as_bytes()); };
    panic::set_hook(Box::new(|_| {
        unsafe {
            // there's probably a less hairy way of doing this, but I'm not rust ninja enough yet
            let mut url_str = str::from_utf8(&PANIC_URL).unwrap();
            url_str = &url_str[0..PANIC_URL.into_iter().position(|r| { r == 0 }).unwrap()];
            open_url_in_chrome_and_exit(&String::from(url_str), &String::default());
        }
    }));

    match LevelFilter::from_str(args.log_level.as_str()) {
        Ok(x) => simple_logging::log_to_file("test.log", x).unwrap(),
        Err(x) => warn!("failed to set log level! logging off {:?}", x),
    }

    if args.create_keys {
        do_utility_and_exit(registry_utils::create_registry_keys);
    } else if args.delete_keys {
        do_utility_and_exit(registry_utils::delete_registry_keys);
    }
    
    let mut chrome = chrome_interface::ChromeInterface::new();
    if !chrome.populate_profile_entries()
    {
        error!("couldn't get chrome profile(s)");
        soft_panic(&args.url);
    }

    let device_state = DeviceState::new();
    let keys: Vec<Keycode> = device_state.get_keys();
    if !args.force_ui && !keys.contains(&Keycode::LControl)
    {
        open_url_in_chrome_and_exit(&args.url, &String::default());
        return;
    }

    // design notes: 
    // - be able to "pin" a profile
    // if ctrl pressed
    //  open UI
    // else
    //  if profile pinned
    //   open in that one
    //  else
    //   open in last used

    let app_height = (chrome.profile_entries.len() as f32) * MyApp::BUTTON_SIZE + 30.0;
    let app_width = MyApp::PROFILE_BUTTON_WIDTH + MyApp::BUTTON_SIZE * 2.0 + 30.0; // profile button + two buttons + margins (5px*3)

    let ci_arcm = Arc::new(Mutex::new(chrome));
    let profile_picture_fetch = ci_arcm.clone();
    for entry in &mut profile_picture_fetch.lock().unwrap().profile_entries {
        let profile_picture_shared = entry.profile_picture.clone();
        let profile_name = entry.profile_name.clone();
        tokio::runtime::Handle::current().spawn(
            async move
            {
                let mut locked_picture = profile_picture_shared.lock().await;
                let fetch_picture_result = locked_picture.get_picture().await;
                if fetch_picture_result.is_err() {
                    warn!("error fetching picture for \"{}\": {}", profile_name, fetch_picture_result.err().unwrap());
                }
            }
        );
    }

    // actually run the app
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(app_width  + 200.0, app_height)),
        resizable: false,
        // decorated: false,
        ..Default::default()
    };

    eframe::run_native(
        "chrome picker",
        options,
        Box::new(|_cc| Box::new(MyApp {chrome_interface: ci_arcm, url: args.url })),
    );
}

struct MyApp {
    chrome_interface: Arc<Mutex<ChromeInterface>>,
    url: String,
}

impl MyApp
{
    const BUTTON_SIZE: f32 = 30.0;
    const PROFILE_BUTTON_WIDTH: f32 = 200.0;
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
            
                                let image = egui::Image::new(texture, egui::vec2(MyApp::BUTTON_SIZE, MyApp::BUTTON_SIZE));
                                ui.add_sized(egui::vec2(MyApp::BUTTON_SIZE, MyApp::BUTTON_SIZE), image);
                            }
                        };
                    }
                });

                ui.with_layout(egui::Layout::top_down(egui::Align::TOP), |ui| {
                    for profile_entry in &self.chrome_interface.lock().unwrap().profile_entries {
                        let mut button = egui::Button::new(profile_entry.profile_name.clone());
                        if ui.add_sized(egui::vec2(200.0, MyApp::BUTTON_SIZE), button).clicked() {
                            open_url_in_chrome_and_exit(&self.url, &profile_entry.profile_directory.clone());
                        }
                    }
                });

                ui.with_layout(egui::Layout::top_down(egui::Align::TOP), |ui| {
                    for profile_entry in &self.chrome_interface.lock().unwrap().profile_entries {
                        let cb = egui::ComboBox::from_id_source(profile_entry.profile_name.as_str()).width(100.0)
                        .show_ui(ui, |ui| {
                            ui.set_min_size(egui::vec2(150.0, 30.0));
                            ui.button("set default".to_string());
                            ui.button("turn off browser check".to_string());
                        });

                        /*
                        let mut button = egui::Button::new("...".to_string());
                        button = button.min_size(egui::vec2(MyApp::BUTTON_SIZE, MyApp::BUTTON_SIZE));
                        if ui.add(button).clicked() {
                            // todo, dropdown menu
                        }
                        */
                    }
                });

            });
        });
    }
}

fn open_url_in_chrome_and_exit(url: &String, profile_name: &String)
{
    debug!("url: {}", url);
    let mut chrome_command_line: String = String::default();
    match registry_utils::get_chrome_exe()
    {
        Err(e) => error!("failed to get chrome open command: {}", e),
        Ok(v) => chrome_command_line = v,
    }

    let mut chrome_command = Command::new(chrome_command_line);
    chrome_command.creation_flags(DETACHED_PROCESS);

    if !profile_name.is_empty() {
        chrome_command.arg(format!("--profile-directory={}", profile_name));
    }
    
    chrome_command.arg("--single-argument").arg(url);

    let chrome_command_child_result = chrome_command.spawn();

    debug!("chrome command: {:?}", chrome_command);

    match chrome_command_child_result
    {
        Err(e) => println!("Error excecuting command: {}", e),
        _ => (),
    };
}
