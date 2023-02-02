use fltk::{app, prelude::*, window::Window, button::Button, text::TextDisplay, text::TextBuffer};
use clap::Parser;
use std::process::Command;

mod chrome_interface;
mod http_utils;
mod registry_utils;

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
    
        let profile_names = my_chrome_interface.get_profile_names().unwrap_or_default();
        
        if !profile_names.contains(&last_used_profile_name)
        {
            println!("last used profile {} not found in profile_names: {:?}", last_used_profile_name, profile_names);
            return;
        }
        // todo: 
        // - surpress default browser warning (Chrome/Default/Preferences: check_default_browser)
        // - enumerate all (chromium?) browsers and handle openers programmatically?

        println!("Open url: {}", args.url);
        let mut chrome_command_line: String = String::default();
        match registry_utils::get_chrome_exe()
        {
            Err(e) => println!("Failed to get chrome open command: {}", e),
            Ok(v) => chrome_command_line = v,
        }

        let mut chrome_command = Command::new("cmd.exe");
        let url_escaped = http_utils::escape_for_cmd(args.url);

        println!("last used profile is: {}", last_used_profile_name);
        chrome_command.arg("/C")
            .arg("start")
            .arg("") // windows is stupid, if the first argument has quotes, it's used as the window title
            .arg(chrome_command_line)
            .arg(format!("--profile-directory={}", last_used_profile_name))
            .arg("--single-argument")
            .arg(url_escaped);

        let chrome_command_child_result = chrome_command.spawn();
    
        println!("command: {:?}", chrome_command);

        match chrome_command_child_result
        {
            Err(e) => println!("Error excecuting command: {}", e),
            _ => (),
        };
            
        // the window
        let app = app::App::default();
        let mut wind = Window::new(100, 100, 400, 300, "Hello from rust");

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
        for profile_name in profile_names
        {
            let mut profile_button = Button::new(20, y, 200, 25, None);
            profile_button.set_label(&profile_name);
            profile_button.set_callback( |w| { 
                let ci = chrome_interface::ChromeInterface::new();
                ci.set_lastused_profile(w.label().as_str()); });
            wind.add(&profile_button);
            y += y_spacing;
        }

        wind.end();
        wind.show();
        app.run().unwrap();
        
    }
}
