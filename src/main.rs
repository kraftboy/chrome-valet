#![windows_subsystem = "windows"]

// #[macro_use] extern crate quick_error;

mod chrome_interface;
mod registry_utils;

use clap::Parser;
use device_query::{DeviceQuery, DeviceState, Keycode};
use eframe::egui;
use log::LevelFilter;
use log::{debug, error, trace, warn};
use std::io;
use std::os::windows::process::CommandExt;
use std::panic;
use std::process::exit;
use std::process::Command;
use std::str;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

use chrome_interface::ChromeInterface;
use registry_utils::Browser;

const DETACHED_PROCESS: u32 = 0x00000008;

fn soft_panic(url: &Option<String>) {
    if url.is_some() {
        open_url_in_chrome(&Browser::Chrome, &url.as_ref().unwrap(), None, true);
    }
}

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Url to open
    #[arg(long)]
    url: Option<String>,

    /// Log level (error, warn, info, debug, trace)
    #[arg(long, default_value = "info")]
    log_level: Option<String>,

    /// force ui to open
    #[arg(long, default_value = "false")]
    force_ui: bool,
}

static mut PANIC_URL: [u8; 2048] = [0; 2048];

fn set_log_level(level: LevelFilter) {
    simple_logging::log_to(io::stdout(), level);
    match simple_logging::log_to_file(
        chrome_interface::app_data_dir().join("chromevalet.log"),
        level,
    ) {
        Err(err) => error!("couldn't log to chromevalet.log: {err}"),
        _ => (),
    }
}

#[tokio::main]
async fn main() {
    // lets us print to the console despite using windows subsystem (ie, process doesn't spawn console)
    // perhaps a to-do is to generate two binaries, one for console, the other not
    #[cfg(target_os = "windows")]
    {
        use winapi::um::wincon::{AttachConsole, ATTACH_PARENT_PROCESS};
        unsafe {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }

    // for tracking startup time
    let main_begin_time: Instant = Instant::now();

    let args = Args::parse();
    set_log_level(LevelFilter::Info);
    if let Some(log_level) = &args.log_level {
        match LevelFilter::from_str(log_level.as_str()) {
            Ok(x) => {
                set_log_level(x);
            }
            Err(x) => warn!("failed to set log level! logging off: {:?}", x),
        }
    }

    debug!("args: {:?}", args);

    if let Some(url) = &args.url {
        // register minimum nice behaviour for panics, just open the damn browser
        unsafe {
            PANIC_URL[0..url.len()].copy_from_slice(url.as_bytes());
        };
        panic::set_hook(Box::new(|_| {
            unsafe {
                // there's probably a less hairy way of doing this, but I'm not rust ninja enough yet
                let mut url_str = str::from_utf8(&PANIC_URL).unwrap();
                url_str = &url_str[0..PANIC_URL.into_iter().position(|r| r == 0).unwrap()];
                open_url_in_chrome(&Browser::Chrome, &String::from(url_str), None, false);
            }
        }));
    }

    let mut chrome = ChromeInterface::new();
    if let Err(e) = chrome.read_prefs() {
        warn!("couldn't read prefs: {}", e);
    }

    // if ctrl pressed or no preferred profile
    //  open UI
    // else
    //  open in preferred profile

    let device_state = DeviceState::new();
    let keys: Vec<Keycode> = device_state.get_keys();
    let preferred_profile = chrome.prefs().get_preferred_profile();
    let default_browser = chrome.get_default_browser();
    if (!args.force_ui && !keys.contains(&Keycode::LAlt)) && args.url.is_some() {
        let preferred_profile = if !preferred_profile.is_empty() {
            Some(&preferred_profile)
        } else {
            None
        };
        open_url_in_chrome(
            &default_browser,
            &args.url.as_ref().unwrap(),
            preferred_profile,
            true,
        );
    }

    if let Err(err) = chrome.populate_profile_entries() {
        error!("couldn't get chrome profile(s): {}", err);
        soft_panic(&args.url);
    }

    let mut app_height = (chrome.profile_entries.len() as f32) * (MyApp::BUTTON_SIZE + 15.0) + 50.0; // need plenty of space for context menu on bottom button
    let app_width = MyApp::PROFILE_BUTTON_WIDTH + MyApp::BUTTON_SIZE * 3.0 + 20.0; // profile button + button + margins (5px*3)

    let mut is_default_browser = true;
    if let Ok(x) = registry_utils::is_default_browser() {
        if !x {
            is_default_browser = false;
            app_height += 100.0; // more height for 'not set as default browser' ui widget
        }
    } else {
        error!("Couldn't do default browser detection");
    }

    let ci_arcm = Arc::new(Mutex::new(chrome));
    let profile_picture_fetch = ci_arcm.clone();
    for entry in &mut profile_picture_fetch.lock().unwrap().profile_entries {
        let profile_picture_shared = entry.profile_picture.clone();
        let profile_name = entry.profile_name.clone();
        tokio::runtime::Handle::current().spawn(async move {
            let mut locked_picture = profile_picture_shared.lock().await;
            let fetch_picture_result = locked_picture.get_picture().await;
            if fetch_picture_result.is_err() {
                warn!(
                    "error fetching picture for \"{}\": {}",
                    profile_name,
                    fetch_picture_result.err().unwrap()
                );
            }
        });
    }

    // actually run the app
    let options = eframe::NativeOptions {
        initial_window_size: Some(eframe::egui::vec2(app_width, app_height)),
        resizable: false,
        centered: true,
        // decorated: false,
        ..Default::default()
    };

    eframe::run_native(
        "Chrome Valet",
        options,
        Box::new(move |_cc| {
            Box::new(MyApp {
                chrome_interface: ci_arcm,
                url: args.url,
                device_state: DeviceState::new(),
                main_begin_time: main_begin_time,
                is_default_browser: is_default_browser,
                default_browser: default_browser,
            })
        }),
    )
    .unwrap();
}

struct MyApp {
    chrome_interface: Arc<Mutex<ChromeInterface>>,
    url: Option<String>,
    device_state: DeviceState,
    main_begin_time: Instant,
    is_default_browser: bool,
    default_browser: Browser,
}

impl MyApp {
    const BUTTON_SIZE: f32 = 30.0;
    const PROFILE_BUTTON_WIDTH: f32 = 200.0;

    fn default_browser_check(&mut self, ui: &mut egui::Ui) {
        if !self.is_default_browser {
            if let Ok(Some(default_browser)) = registry_utils::get_default_browser() {
                let ci_lock = self.chrome_interface.lock();
                let mut ci = ci_lock.unwrap();
                let mut prefs = ci.prefs_mut();
                if let Ok(browser) = Browser::try_from(&prefs.default_browser) {
                    if browser == Browser::Unknown {
                        prefs.default_browser = default_browser.to_string();
                        if let Err(err) = ci.write_prefs() {
                            error!("Failed to write prefs: {err}");
                        }
                    }
                }
            }

            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                ui.scope(|ui| {
                    ui.style_mut().visuals.override_text_color =
                        Some(egui::Color32::from_rgba_unmultiplied(255, 123, 0, 255));
                    ui.add_sized(
                        egui::vec2(240.0, 50.0),
                        egui::Label::new("Chrome Valet must be set as default browser to work.")
                            .wrap(true),
                    );
                });
                if ui
                    .add(egui::Button::new("Open default app settings").wrap(true))
                    .clicked()
                {
                    open_default_apps();
                }
            });

            ui.separator();
        }
    }

    fn show_url(&mut self, ui: &mut egui::Ui) {
        // show the user which url we're talking about
        if let Some(url) = self.url.clone() {
            let trimmed_url_for_display = url.clone();
            let trim_len = 32;
            let trimmed_url_for_display = if trimmed_url_for_display.len() > trim_len {
                &trimmed_url_for_display[0..trim_len]
            } else {
                trimmed_url_for_display.as_str()
            };

            ui.with_layout(egui::Layout::left_to_right(egui::Align::LEFT), |ui| {
                ui.scope(|ui| {
                    ui.style_mut().override_text_style = Some(egui::style::TextStyle::Monospace);
                    let my_label = egui::Label::new(format!("URL: {trimmed_url_for_display}"));
                    ui.add(my_label).on_hover_ui(|ui| {
                        ui.add_sized(egui::vec2(300.0, 100.0), egui::Label::new(&url));
                    });

                    let clipboard_label = egui::Label::new("📋").sense(egui::Sense::click());
                    if ui.add(clipboard_label).clicked() {
                        cli_clipboard::set_contents(url).unwrap();
                    }
                });
            });
        }
    }
}

impl eframe::App for MyApp {
    fn persist_native_window(&self) -> bool {
        false
    }

    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        eframe::egui::CentralPanel::default().show(ctx, |ui| {
            self.default_browser_check(ui);

            self.show_url(ui);

            ui.separator();

            egui::Grid::new("profile_grid").show(ui, |ui| {
                ui.label("");

                let mut default_browser_name = self.default_browser.to_string();
                if let Some(browser_name) = default_browser_name.get_mut(0..1) {
                    browser_name.make_ascii_uppercase();
                }

                ui.label(format!("{} Profile", default_browser_name));
                ui.end_row();

                let chrome_lock = self.chrome_interface.lock();
                if chrome_lock.is_err() {
                    error!("couldn't lock chrome_inteface!");
                    return;
                }

                let mut chrome_interface = chrome_lock.unwrap();
                let prefs = chrome_interface.prefs();
                let last_preferred_profile = prefs.get_preferred_profile();
                let mut preferred_profile = last_preferred_profile.clone();

                for profile_entry in &chrome_interface.profile_entries {
                    let mut lock = profile_entry.profile_picture.try_lock();
                    if let Some(ref mut _mutex) = lock {
                        let mut profile_picture = lock.unwrap();
                        if profile_picture.img.is_some() {
                            let profile_image_copy = profile_picture.img.clone();
                            let texture =
                                profile_picture.profile_texture.get_or_insert_with(|| {
                                    trace!(
                                        "Time until profile texture load: {:5} millis",
                                        self.main_begin_time.elapsed().as_millis()
                                    );

                                    // Load the texture only once.
                                    ui.ctx().load_texture(
                                        format!(
                                            "{} Profile Pic Texture",
                                            profile_entry.profile_name
                                        ),
                                        profile_image_copy.unwrap(),
                                        Default::default(),
                                    )
                                });

                            let image = egui::Image::new(
                                texture,
                                egui::vec2(MyApp::BUTTON_SIZE, MyApp::BUTTON_SIZE),
                            );
                            ui.add_sized(egui::vec2(MyApp::BUTTON_SIZE, MyApp::BUTTON_SIZE), image);
                        } else {
                            let button = egui::Button::new("error")
                                .min_size(egui::vec2(MyApp::BUTTON_SIZE, MyApp::BUTTON_SIZE));
                            ui.add_enabled(false, button);
                        }
                    };

                    let mut button = egui::Button::new(profile_entry.profile_name.clone());

                    // if there's no url, the buttons do nothing
                    if self.url.is_none() {
                        button = button.sense(egui::Sense::hover());
                    }

                    if ui
                        .add_sized(egui::vec2(200.0, MyApp::BUTTON_SIZE), button)
                        .clicked()
                    {
                        let mut exit_after_open_url = true;
                        let keys: Vec<Keycode> = self.device_state.get_keys();
                        if keys.contains(&Keycode::LShift) {
                            exit_after_open_url = false;
                        }

                        open_url_in_chrome(
                            &self.default_browser,
                            &self.url.as_ref().unwrap(),
                            Some(&profile_entry.profile_directory.clone()),
                            exit_after_open_url,
                        );
                    }

                    ui.scope(|ui| {
                        if preferred_profile == profile_entry.profile_directory {
                            ui.style_mut().visuals.override_text_color =
                                Some(egui::Color32::from_rgba_unmultiplied(255, 0, 0, 196));
                        }

                        let button = egui::widgets::Button::new("♡");
                        if ui
                            .add_sized(egui::vec2(MyApp::BUTTON_SIZE, MyApp::BUTTON_SIZE), button)
                            .clicked()
                        {
                            preferred_profile = profile_entry.profile_directory.clone();
                        }
                    });

                    ui.end_row();
                } // for profile entry

                if last_preferred_profile != preferred_profile {
                    let prefs = chrome_interface.prefs_mut();
                    prefs.set_preferred_profile(&preferred_profile);

                    // todo: do this right in prefs once I pull out all the file stuff
                    if let Err(e) = chrome_interface.write_prefs() {
                        error!("couldn't write prefs: {}", e);
                    }
                }
            }); // grid
        });
    }
}

fn open_url_in_chrome(
    browser: &Browser,
    url: &String,
    profile_name: Option<&String>,
    exit_when_done: bool,
) {
    debug!("url: {}", url);
    let mut chrome_command_line: String = String::default();
    match registry_utils::get_browser_exe(browser) {
        Err(e) => error!("failed to get browser exe location: {}", e),
        Ok(v) => chrome_command_line = v,
    }

    // todo: break the open commands by argument, keep them in order, replace the one with %1 with the url
    // for now we assume all chromium browsers play nice with these arguments

    let mut chrome_command = Command::new(chrome_command_line);
    chrome_command.creation_flags(DETACHED_PROCESS);

    if profile_name.is_some() {
        chrome_command.arg(format!("--profile-directory={}", profile_name.unwrap()));
    }

    chrome_command.arg("--single-argument").arg(url);

    let chrome_command_child_result = chrome_command.spawn();

    debug!("chrome command: {:?}", chrome_command);

    if let Err(e) = chrome_command_child_result {
        error!("Error excecuting command: {}", e);
    };

    if exit_when_done {
        exit(0);
    }
}

///
/// opens the windows defauls apps panel
///
fn open_default_apps() {
    let mut default_apps_command = Command::new("cmd");
    default_apps_command.args(["/c", "start", "ms-settings:defaultapps"]);
    default_apps_command.creation_flags(DETACHED_PROCESS);
    let default_apps_command_result = default_apps_command.spawn();
    if let Err(e) = default_apps_command_result {
        error!("Error excecuting command: {}", e);
    };
}
