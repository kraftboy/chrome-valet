#[allow(unused_imports)]

use std::env;
use serde_json::{Value};
use std::path::{Path, PathBuf};
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::Write;
use std::sync::Arc;
use egui::{ColorImage, Color32};
use futures::lock::Mutex;

const LOCALAPPDATA:&str = "LOCALAPPDATA";

pub struct ChromeProfilePicture
{
    picture_filename: OsString,
    pub img: Option<ColorImage>,
    pub profile_texture: Option<egui::TextureHandle>,
    pub profile_color: [u8; 4],
}

impl ChromeProfilePicture
{
    pub fn new(profile_dir: &String, img_filename: &OsString, profile_color: &[u8;4]) -> Self
    {
        let local_app_data = env::var(LOCALAPPDATA).unwrap();
        let mut filename_path = PathBuf::default();
        if !img_filename.is_empty() {
            filename_path = PathBuf::from(&local_app_data);
            filename_path = filename_path.join("Google").join("Chrome").join("User Data").join(profile_dir).join(img_filename);
        }

        ChromeProfilePicture {
            picture_filename: OsString::from(filename_path.as_os_str()), 
            img: None,
            profile_texture: None,
            profile_color: profile_color.clone(),
        }
    }
}

impl ChromeProfilePicture {

    fn apply_circle_mask(self: &mut Self)
    {
        if self.img.is_none() {
            return;
        }

        let img_ref = self.img.as_mut().unwrap();
        assert_eq!(img_ref.width(), img_ref.height(), "not square!");
        let dim = img_ref.width() as i32;
        let half_dim = (img_ref.width()/2) as i32;
        let mut x: i32 = 0;
        let mut y: i32 = 0;
        for pixel in &mut img_ref.pixels
        {
            let x_from_mid = x - half_dim;
            let y_from_mid = y - half_dim;
            let mut dist = (x_from_mid.pow(2)+ y_from_mid.pow(2)) as f32;
            dist = dist.sqrt();
            
            if dist > (half_dim as f32)
            {
                *pixel = Color32::from_rgba_unmultiplied(pixel.r(), pixel.g(), pixel.b(), 0).clone();
            }

            if x == 0
            {
                y += 1;
            }

            x += 1;
            x = x%dim;
        }
    }
    pub async fn get_picture(self: &mut Self) -> Result<(), image::ImageError>
    {
        if !self.picture_filename.is_empty()
        {
            fn load_image_from_path(path: &std::path::Path) -> Result<Option<egui::ColorImage>, image::ImageError> {
                let image = image::io::Reader::open(path)?.decode()?;
                let size = [image.width() as _, image.height() as _];
                let image_buffer = image.to_rgba8();
                let pixels = image_buffer.as_flat_samples();
                Ok(Some(egui::ColorImage::from_rgba_unmultiplied(
                    size,
                    pixels.as_slice(),
                )))
            }

            self.img = load_image_from_path(Path::new(&self.picture_filename)).unwrap();
        } else {
            // make an image entirely with the color of the profile
            let image_size = 128;
            let red_pixels = self.profile_color.iter().cloned().cycle();
            let image_array = Vec::from(red_pixels.take(image_size * image_size * self.profile_color.len()).collect::<Vec<u8>>());
            self.img = Some(ColorImage::from_rgba_unmultiplied([image_size, image_size], &image_array));
        }

        self.apply_circle_mask();

        return Ok(());
    }
}

#[derive(Debug)]
pub struct ChromeProfileEntry
{
    pub profile_directory: String,
    pub profile_name: String,
    pub profile_picture: Arc<Mutex<ChromeProfilePicture>>,
}

unsafe impl Send for ChromeProfilePicture {}

#[derive(Default)]
pub struct ChromeInterface
{
    statefile_path: OsString,
    pub profile_entries: Vec<ChromeProfileEntry>
}

impl ChromeInterface
{
    pub fn new() -> Self
    {
        let local_app_data = env::var(LOCALAPPDATA).unwrap();
        let new = ChromeInterface {
            statefile_path : Path::join(Path::new(OsStr::new(&local_app_data)),"Google/Chrome/User Data/Local State").into_os_string(),
            profile_entries: Vec::new()
        };
        return new;
    }

    fn prefs_path(profile_dir: &String) -> PathBuf
    {
        let local_app_data = env::var(LOCALAPPDATA).unwrap();
        return PathBuf::from(local_app_data).join("Google").join("Chrome").join(profile_dir).join("Preferences");
    }

    fn open_local_statefile_as_object(&self) -> Value
    {
        let statefile_reader = File::open(self.statefile_path.to_os_string()).unwrap();
        return serde_json::from_reader(statefile_reader).unwrap();
    }

    fn open_prefs_as_object(profile_dir: &String) -> Value
    {
        let prefs_path = Self::prefs_path(profile_dir);
        let prefs_reader = File::open(prefs_path.as_os_str().to_os_string()).unwrap();
        return serde_json::from_reader(prefs_reader).unwrap();
    }

    pub fn populate_profile_entries(&mut self) -> bool
    {
        let local_statefile_obj = self.open_local_statefile_as_object();
        let json_profiles = &local_statefile_obj["profile"]["info_cache"];
        if json_profiles.is_object()
        {
            for profile_entry in json_profiles.as_object().unwrap()
            {   
                let entry_data = profile_entry.1.as_object().unwrap();
                let mut profile_filename = OsString::default();
                if entry_data.get_key_value("gaia_picture_file_name").is_some() {
                    profile_filename = OsString::from(entry_data.get_key_value("gaia_picture_file_name").unwrap().1.as_str().unwrap_or_default());
                }

                let mut profile_color: [u8; 4] = [0,0,0,0];
                if entry_data.get_key_value("default_avatar_fill_color").is_some() {
                    let colour = entry_data.get_key_value("default_avatar_fill_color").unwrap().1.as_i64().unwrap_or_default();
                    // color is argb
                    profile_color[3] = ((colour >> 24) & 0xff) as u8;
                    profile_color[0] = ((colour >> 16) & 0xff) as u8;
                    profile_color[1] = ((colour >> 8) & 0xff) as u8;
                    profile_color[2] = ((colour >> 0) & 0xff) as u8;
                }
         
                let chrome_profile_entry: ChromeProfileEntry = ChromeProfileEntry {
                    profile_directory: profile_entry.0.to_string(),
                    profile_name: String::from(entry_data["shortcut_name"].as_str().unwrap()),
                    profile_picture: Arc::new(Mutex::new(
                        ChromeProfilePicture::new(
                        &profile_entry.0.to_string(),
                        &profile_filename,
                        &profile_color)
                    )),
                };

                self.profile_entries.push(chrome_profile_entry);
            }

            return true;
        }
        
        return false;
    }

    #[allow(dead_code)]
    pub fn set_lastused_profile(&self, profile_name:&str)
    {
        let local_app_data = env::var(LOCALAPPDATA).unwrap();
        let statefile_path = Path::join(Path::new(OsStr::new(&local_app_data)), "Google/Chrome/User Data/Local State").into_os_string();
        
        let local_statefile_obj = self.open_local_statefile_as_object();
        let mut local_statefile_changed = local_statefile_obj;
        local_statefile_changed["profile"]["last_used"] = Value::from(profile_name.to_string());

        let mut statefile_write = File::options().write(true).truncate(true).open(statefile_path.clone()).unwrap();
        statefile_write.write(local_statefile_changed.to_string().as_bytes()).unwrap();
    }
    
    pub fn set_chrome_default_browser_check(&self, profile_dir: &String, check: bool)
    {
        let mut prefs_obj = Self::open_prefs_as_object(profile_dir);
        prefs_obj["browser"]["default_browser_setting_enabled"] = Value::from(check);

        // todo, make statefile/prefs objects that open/close on new and drop
        let prefs_path = Self::prefs_path(profile_dir);
        let mut statefile_write = File::options().write(true).truncate(true).open(prefs_path.clone()).unwrap();
        statefile_write.write(prefs_obj.to_string().as_bytes()).unwrap();
    }
}




