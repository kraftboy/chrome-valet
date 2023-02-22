
use std::fs;
use std::env;
use serde::{Serialize, Deserialize};
use serde_json::{Value};
use std::path::{Path, PathBuf};
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{Error as IoErr, Write, Result as IoResult, ErrorKind as IoErrorKind};
use std::sync::Arc;
use egui::{ColorImage, Color32};
use futures::lock::Mutex;
use log::{error};
use std::fmt::Display;

use crate::registry_utils;

const LOCALAPPDATA:&str = "LOCALAPPDATA";
const PROGRAM_NAME:&str = "ChromeValet";

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
            profile_color: *profile_color,
        }
    }
}

impl ChromeProfilePicture {

    fn apply_circle_mask(&mut self)
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
                *pixel = Color32::from_rgba_unmultiplied(pixel.r(), pixel.g(), pixel.b(), 0);
            }

            if x == 0
            {
                y += 1;
            }

            x += 1;
            x %= dim;
        }
    }
    pub async fn get_picture(&mut self) -> Result<(), image::ImageError>
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
            let image_array = red_pixels.take(image_size * image_size * self.profile_color.len()).collect::<Vec<u8>>();
            self.img = Some(ColorImage::from_rgba_unmultiplied([image_size, image_size], &image_array));
        }

        self.apply_circle_mask();

        Ok(())
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

#[derive(Default, Serialize, Deserialize)]
pub struct ProgramPrefs
{
    #[serde(default)]
    pub preferred_profile: String,

    #[serde(default)]
    pub default_browser: String
}

impl ProgramPrefs
{
    pub fn get_preferred_profile(&self) -> String {
        self.preferred_profile.to_owned()
    }

    pub fn set_preferred_profile(&mut self, profile_dir: &str)
    {
        self.preferred_profile = profile_dir.to_string();
    }
}

#[derive(Debug)]
pub enum ChromeInterfaceError
{
    IoError(IoErr),
    JsonFormatError(String),
}

impl Display for ChromeInterfaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChromeInterfaceError::IoError(io_err) => 
                write!(f, "{}", io_err),
                ChromeInterfaceError::JsonFormatError(errstr) => 
                write!(f, "{}", errstr),
        }
    }
}

#[derive(Default)]
pub struct ChromeInterface
{
    statefile_path: OsString,
    pub profile_entries: Vec<ChromeProfileEntry>,
    prefs: ProgramPrefs,
}

impl ChromeInterface
{
    pub fn new() -> Self
    {
        let local_app_data = env::var(LOCALAPPDATA).unwrap();
        let browser_appdata_dir = registry_utils::Browser::Brave.get_definition().unwrap().app_data_dir;
        let mut chrome_interface = ChromeInterface {
            statefile_path : Path::join(Path::new(OsStr::new(&local_app_data)),format!("{browser_appdata_dir}/User Data/Local State")).into_os_string(),
            profile_entries: Vec::new(),
            prefs: ProgramPrefs::default(),
        };

        if let Err(err) = chrome_interface.read_prefs()
        {
            error!("failed to read prefs: {err}");
        }

        
        chrome_interface
    }

    pub fn get_default_browser(&mut self) -> registry_utils::Browser
    {

        if let Ok(Some(browser)) = registry_utils::get_default_browser() {
            if self.prefs().default_browser != browser.to_string()
            {
                self.prefs_mut().default_browser = browser.to_string();
                if let Err(err) = self.write_prefs()
                {
                    error!("error writing prefs: {err}");
                }
            }
        }

        let default_browser_string = self.prefs().default_browser.to_owned();
        let mut default_browser = registry_utils::Browser::try_from(&default_browser_string);
        if default_browser.is_err() {
            default_browser = Ok(registry_utils::Browser::Chrome);
        }

        return default_browser.unwrap();
    }

    pub fn prefs(&self) -> &ProgramPrefs
    {
        &self.prefs
    }

    pub fn prefs_mut(&mut self) -> &mut ProgramPrefs
    {
        &mut self.prefs
    }

    pub fn app_data_dir() -> PathBuf
    {
        let local_app_data = env::var(LOCALAPPDATA).unwrap();
        PathBuf::from(local_app_data).join(PROGRAM_NAME)
    }
    fn my_prefs_path() -> PathBuf
    {
        ChromeInterface::app_data_dir().join("prefs.json")
    }

    fn chrome_prefs_path(profile_dir: &String) -> PathBuf
    {
        let local_app_data = env::var(LOCALAPPDATA).unwrap();
        PathBuf::from(local_app_data).join("Google").join("Chrome").join(profile_dir).join("Preferences")
    }

    fn open_file_as_object(filepath: &OsString) -> IoResult<Value>
    {
        let reader = File::open(filepath)?;
        Ok(serde_json::from_reader(reader)?)
    }

    fn open_local_statefile_as_object(&self) -> IoResult<Value>
    {
        Self::open_file_as_object(&self.statefile_path)
    }

    fn open_prefs_as_object(profile_dir: &String) -> IoResult<Value>
    {
        let prefs_path = Self::chrome_prefs_path(profile_dir);
        return Self::open_file_as_object(&prefs_path.as_os_str().to_os_string());
    }

    fn write_to_file(file_path: &Path, file_contents: &[u8]) -> IoResult<()>
    {
        if let Some(p) = file_path.parent() {
            fs::create_dir_all(p)?
        };

        let mut writer = File::options().write(true).truncate(true).create(true).open(file_path)?;
        _ = writer.write(file_contents)?;

        Ok(())
    }

    fn write_value_to_file(file_path: &Path, file_contents: &Value) -> IoResult<()>
    {
        Self::write_to_file(file_path, file_contents.to_string().as_bytes())
    }

    pub fn populate_profile_entries(&mut self) -> Result<(), ChromeInterfaceError>
    {
        let local_statefile_result = self.open_local_statefile_as_object();
        if local_statefile_result.is_err() 
        {
            return Err(ChromeInterfaceError::IoError(local_statefile_result.err().unwrap()))
        }

        let mut json_profiles = &local_statefile_result.unwrap();
        if json_profiles.is_object()
        {
            json_profiles = &json_profiles["profile"]["info_cache"];
            for profile_entry in json_profiles.as_object().unwrap()
            {   
                let entry_data = profile_entry.1.as_object().unwrap();
                let profile_filename = match entry_data.get_key_value("gaia_picture_file_name") {
                    Some(e) => OsString::from(e.1.as_str().unwrap_or_default()),
                    None => OsString::default(),
                };

                let mut profile_color: [u8; 4] = [0,0,0,0];
                if let Some(e) = entry_data.get_key_value("default_avatar_fill_color") {
                        let colour = e.1.as_i64().unwrap_or_default();
                        // color is argb
                        profile_color[3] = ((colour >> 24) & 0xff) as u8;
                        profile_color[0] = ((colour >> 16) & 0xff) as u8;
                        profile_color[1] = ((colour >> 8) & 0xff) as u8;
                        profile_color[2] = (colour & 0xff) as u8;
                };
         
                let shortcut_name = match entry_data.get_key_value("shortcut_name") {
                    Some(e) => e.1.as_str().unwrap_or_default(),
                    None => "",
                };

                let chrome_profile_entry: ChromeProfileEntry = ChromeProfileEntry {
                    profile_directory: profile_entry.0.to_string(),
                    profile_name: shortcut_name.to_string(),
                    profile_picture: Arc::new(Mutex::new(
                        ChromeProfilePicture::new(
                        &profile_entry.0.to_string(),
                        &profile_filename,
                        &profile_color)
                    )),
                };

                self.profile_entries.push(chrome_profile_entry);
            }

            return Ok(());
        }
        
        Err(ChromeInterfaceError::JsonFormatError(format!("json_profiles not object: {}", json_profiles.to_string())))
    }

    pub fn read_prefs(&mut self) -> IoResult<()>
    {
        let reader_result = File::open(Self::my_prefs_path().as_os_str());
        match reader_result {
            Ok(x) => self.prefs = serde_json::from_reader(x)?,
            Err(e) => match e.kind() {
                IoErrorKind::NotFound => return Ok(()),
                y => return Err(std::io::Error::from(y)),
            },
        }
        
        Ok(())
    }

    pub fn write_prefs(&self) -> IoResult<()>
    {
        let prefs_string = serde_json::to_string(&self.prefs).unwrap();
        let prefs_bytes = prefs_string.as_bytes();
        Self::write_to_file(Self::my_prefs_path().as_path(), prefs_bytes)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn set_lastused_profile(&self, profile_name:&str)
    {
        let local_app_data = env::var(LOCALAPPDATA).unwrap();
        let statefile_path = Path::join(Path::new(OsStr::new(&local_app_data)), "Google/Chrome/User Data/Local State").into_os_string();
        
        let local_statefile_obj = self.open_local_statefile_as_object();
        let mut local_statefile_changed = local_statefile_obj.unwrap();
        local_statefile_changed["profile"]["last_used"] = Value::from(profile_name.to_string());

        let mut statefile_write = File::options().write(true).truncate(true).open(statefile_path).unwrap();
        _ = statefile_write.write(local_statefile_changed.to_string().as_bytes()).unwrap();

    }
    
    #[allow(dead_code)]
    pub fn set_chrome_default_browser_check(profile_dir: &String, check: bool) -> IoResult<()>
    {
        let mut prefs_obj = Self::open_prefs_as_object(profile_dir)?;
        prefs_obj["browser"]["default_browser_setting_enabled"] = Value::from(check);

        // todo, make statefile/prefs objects that open/close on new and drop
        Self::write_value_to_file(&Self::chrome_prefs_path(profile_dir), &prefs_obj)?;
        Ok(())
    }
}




