#![windows_subsystem = "windows"]

use fltk::{app, prelude::*, window::Window, button::Button, text::TextDisplay, text::TextBuffer};
use fltk_theme::{WidgetTheme, ThemeType};
use device_query::{DeviceQuery, DeviceState, Keycode};
use std::sync::Once;
use std::{thread, time};

use clap::Parser;
use std::process::Command;
use std::os::windows::process::CommandExt;

mod chrome_interface;
mod registry_utils;

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

fn main() {

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
        let my_chrome_interface = chrome_interface::ChromeInterface::new(); 
    
        let last_used_profile_name = match my_chrome_interface.get_lastused_profile()
        {
            Some(x) => x,
            None => {
                "No profile found".to_string();
                return;
            },
        };
    
        let profile_entries = my_chrome_interface.get_profile_names().unwrap_or_default();
        
        if !profile_entries.iter().any(|x| { return x.profile_directory == last_used_profile_name; })
        {
            let debug_profile_names = profile_entries.clone();
            println!("last used profile {} not found in profile_names: {:?}", last_used_profile_name, debug_profile_names);
            return;
        }
        // todo: 
        // - surpress default browser warning (Chrome/Default/Preferences: check_default_browser)
        // - enumerate all (chromium?) browsers and handle openers programmatically?

        // the window
        let app = app::App::default();
        let widget_theme = WidgetTheme::new(ThemeType::Dark);
        widget_theme.apply();
        let mut wind = Window::new(100, 100, 240, 300, "Chromecierge");

        let mut y = 20;
        let y_spacing = 30;

        // last used profile display
        let mut my_last_used_profile_display = TextDisplay::new(20, y, 200, 25, None);
        let mut my_textbuffer: TextBuffer = TextBuffer::default();
        my_textbuffer.set_text(last_used_profile_name.as_str());
        my_last_used_profile_display.set_buffer(my_textbuffer);
        wind.add(&my_last_used_profile_display);

        y+=y_spacing;

        // available profile names
        for profile_entry in profile_entries
        {
            let mut profile_button = Button::new(20, y, 200, 25, None);
            profile_button.set_label(&profile_entry.profile_name);
            let cb_url = args.url.clone();
            profile_button.set_callback(move |_w| { 
                open_url_in_chrome(&cb_url, &profile_entry.profile_directory);
                app.quit();
            });
            wind.add(&profile_button);
            y += y_spacing;
        }

        wind.end();
        wind.show(); // need show for idle handler to run, hrm ... don't want to blink window for one frame

        let sleep_interval = time::Duration::from_millis(200);
        let check_on_startup: Once = Once::new();
        app::add_idle3(move |_x|
        {
            let device_state = DeviceState::new();
            let keys: Vec<Keycode> = device_state.get_keys();
            check_on_startup.call_once(|| {
                if !keys.contains(&Keycode::LControl)
                {
                    open_url_in_chrome(&args.url, &last_used_profile_name);
                    app.quit();
                }
            });

            thread::sleep(sleep_interval);
        });

        app.run().unwrap();
        
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

    chrome_command
        .arg(format!("--profile-directory={}", profile_name))
        .arg("--single-argument")
        .arg(url);

    let chrome_command_child_result = chrome_command.spawn();

    println!("command: {:?}", chrome_command);

    match chrome_command_child_result
    {
        Err(e) => println!("Error excecuting command: {}", e),
        _ => (),
    };
}
