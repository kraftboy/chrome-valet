#![windows_subsystem = "windows"]

// #[macro_use] extern crate quick_error;

use std::os::windows::process::CommandExt;
use std::process::Command;
use std::process::exit;
use std::sync::Arc;
use std::sync::Mutex;
use std::str::FromStr;
use std::panic;
use std::str;
use log::LevelFilter;
use log::{debug,warn,info,error,trace};
use std::time::Instant;

use eframe::egui;
use device_query::{DeviceQuery, DeviceState, Keycode};
use egui::{Color32,Sense};
use clap::Parser;
use std::io;

mod registry_utils;
mod chrome_interface;

const DETACHED_PROCESS: u32 = 0x00000008;

type RegUtilityFn = fn(bool) -> Result<(), registry::Error>;
fn do_utility_and_exit(util_fn: RegUtilityFn, create: bool, action: &str)
{
    match util_fn(create)
    {
        Err(e) => error!("Failed: {} -> {:?}", action, e),
        _ => info!("{}", action),
    }

    exit(0);
}

fn soft_panic(url: &String)
{
    open_url_in_chrome_and_exit(&url, &String::default(), true);
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

#[tokio::main]
async fn main() {

    // lets us print to the console despite using windows subsystem (ie, process doesn't spawn console)
    // perhaps a to-do is to generate two binaries, one for console, the other not
    /*
    #[cfg(target_os = "windows")]
    {
        use winapi::um::wincon::{AttachConsole, ATTACH_PARENT_PROCESS};
        unsafe {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
    */
    // for tracking startup time
    let main_begin_time: Instant = Instant::now();

    let args = Args::parse();
    match LevelFilter::from_str(args.log_level.as_str()) {
        Ok(x) => simple_logging::log_to(io::stdout(), x),
        Err(x) => warn!("failed to set log level! logging off {:?}", x),
    }

    debug!("args: {:?}", args);
    // register minimum nice behaviour for panics, just open the damn browser
    unsafe { PANIC_URL[0..args.url.len()].copy_from_slice(args.url.as_bytes()); };
    panic::set_hook(Box::new(|_| {
        unsafe {
            // there's probably a less hairy way of doing this, but I'm not rust ninja enough yet
            let mut url_str = str::from_utf8(&PANIC_URL).unwrap();
            url_str = &url_str[0..PANIC_URL.into_iter().position(|r| { r == 0 }).unwrap()];
            open_url_in_chrome_and_exit(&String::from(url_str), &String::default(), false);
        }
    }));

    if args.create_keys {
        do_utility_and_exit(registry_utils::edit_registry_keys, true, "Registry keys create");
    } else if args.delete_keys {
        do_utility_and_exit(registry_utils::edit_registry_keys, false, "Registry keys delete");
    }
    
    let mut chrome = chrome_interface::ChromeInterface::new();
    match chrome.read_prefs() {
        Err(e) => warn!("couldn't read prefs: {}", e),
        _ => (),
    }

    if !chrome.populate_profile_entries()
    {
        error!("couldn't get chrome profile(s)");
        soft_panic(&args.url);
    }

    let device_state = DeviceState::new();
    let keys: Vec<Keycode> = device_state.get_keys();
    let preferred_profile = chrome.prefs().get_preferred_profile();
    if (!args.force_ui && !keys.contains(&Keycode::LControl)) && !args.url.is_empty() && !preferred_profile.is_empty()
    {
        open_url_in_chrome_and_exit(&args.url, &preferred_profile, true);
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

    let app_height = (chrome.profile_entries.len() as f32) * (MyApp::BUTTON_SIZE + 15.0) + 50.0; // need plenty of space for context menu on bottom button
    let app_width = MyApp::PROFILE_BUTTON_WIDTH + MyApp::BUTTON_SIZE * 3.0 + 20.0; // profile button + button + margins (5px*3)

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
        initial_window_size: Some(egui::vec2(app_width, app_height)),
        resizable: false,
        // decorated: false,
        ..Default::default()
    };

    eframe::run_native(
        "chrome picker",
        options,
        Box::new(move |_cc|
            Box::new(MyApp {chrome_interface: ci_arcm,
                url: args.url,
                device_state: DeviceState::new(),
                main_begin_time
                : main_begin_time })),
    );
    
}

struct MyApp {
    chrome_interface: Arc<Mutex<chrome_interface::ChromeInterface>>,
    url: String,
    device_state: DeviceState,
    main_begin_time: Instant,
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
        
            // show the user which url we're talking about
            if !self.url.is_empty() {
                let mut trimmed_url_for_display = self.url.clone();
                if trimmed_url_for_display.len() > 32 {
                    trimmed_url_for_display = format!("{}...", trimmed_url_for_display[0..32].to_string());
                }
                ui.label(format!("url to open: {}", trimmed_url_for_display))
                    .on_hover_ui(|ui| {
                        ui.add_sized(egui::vec2(300.0, 100.0), egui::Label::new(self.url.clone()));
                    });
            }
            
            egui::Grid::new("some_unique_id").show(ui, |ui| {

                ui.label("");
                ui.label("Profile");
                ui.end_row();

                let chrome_lock = self.chrome_interface.lock();
                if chrome_lock.is_err() {
                    error!("couldn't lock chrome_inteface!");
                    return;
                }

                let mut chrome_interface = chrome_lock.unwrap();
                let prefs = chrome_interface.prefs();
                let last_preferred_profile = prefs.get_preferred_profile().clone();
                let mut preferred_profile = last_preferred_profile.clone();

                for profile_entry in &chrome_interface.profile_entries {
                    let mut lock = profile_entry.profile_picture.try_lock();
                    if let Some(ref mut _mutex) = lock {
                        let mut profile_picture = lock.unwrap();
                        if profile_picture.img != None
                        {
                            let profile_image_copy = profile_picture.img.clone();
                            let texture: &egui::TextureHandle = profile_picture.profile_texture.get_or_insert_with(|| {

                                trace!("Time until profile texture load: {:5} millis", self.main_begin_time.elapsed().as_millis());

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

                    /*
                    // I would love to use context menus, but they have a major problem right now in that they paint off the edges
                    // of the frame. Comboboxes are not much better. They do reposition - albeit kind of stupidly - but you can't
                    // set the size of the "button" that fronts them (outside of a bunch of manual stuff out of the scope of this effort)
                    // I'll leave this here tho:
                    .context_menu(|ui| {
                        ui.button("set nickname".to_string());
                        ui.button("turn off browser check".to_string());
                    })
                     */
                    let mut button = egui::Button::new(profile_entry.profile_name.clone());
                    
                    // if there's no url, the buttons do nothing
                    if self.url.is_empty() {
                        button = button.sense(Sense::hover());
                    }

                    if ui.add_sized(egui::vec2(200.0, MyApp::BUTTON_SIZE), button).clicked() {
                        
                        let mut exit_after_open_url = true;
                        let keys: Vec<Keycode> = self.device_state.get_keys();
                        if keys.contains(&Keycode::LShift)
                        {
                            exit_after_open_url = false;
                        }

                        open_url_in_chrome_and_exit(&self.url, &profile_entry.profile_directory.clone(), exit_after_open_url);
                    }
                    
                    ui.scope(|ui| {
                        if preferred_profile == profile_entry.profile_directory {
                            ui.style_mut().visuals.override_text_color = Some(Color32::from_rgba_unmultiplied(255, 0, 0, 196));
                        }

                        let button = egui::widgets::Button::new("♡");
                        if ui.add_sized(egui::vec2(MyApp::BUTTON_SIZE, MyApp::BUTTON_SIZE), button).clicked() {
                            // todo: set default
                            preferred_profile = profile_entry.profile_directory.clone();
                        }
                    });

                    ui.end_row();
                } // for profile entry

                if last_preferred_profile != preferred_profile
                {
                    let prefs = chrome_interface.prefs_mut();
                    prefs.set_preferred_profile(&preferred_profile);

                    // todo: do this right in prefs once I pull out all the file stuff
                    match chrome_interface.write_prefs() {
                        Err(e) => error!("couldn't write prefs: {}", e),
                        _ => {}
                    }
                }
            });
        });
    }
}

fn open_url_in_chrome_and_exit(url: &String, profile_name: &String, exit_when_done: bool)
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
        Err(e) => error!("Error excecuting command: {}", e),
        _ => (),
    };

    if exit_when_done {
        exit(0);
    }
}
