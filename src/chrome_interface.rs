#[allow(unused_imports)]

use std::env;
use serde_json::{Value};
use std::path::Path;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::Write;

const LOCALAPPDATA:&str = "LOCALAPPDATA";

pub struct ChromeInterface
{
    statefile_path: OsString
}

impl ChromeInterface
{
    pub fn new() -> Self
    {
        let local_app_data = env::var(LOCALAPPDATA).unwrap();
        Self { statefile_path : Path::join(Path::new(OsStr::new(&local_app_data)), "Google/Chrome/User Data/Local State").into_os_string() }
    }

    fn open_local_statefile_as_object(&self) -> Value
    {
        let statefile_reader = File::open(self.statefile_path.to_os_string()).unwrap();
        return serde_json::from_reader(statefile_reader).unwrap();
    }

    pub fn get_lastused_profile(&self) -> Option<String>
    {
        let local_statefile_obj = self.open_local_statefile_as_object();
        // important to get as_str as going to_string will represent value with doublequotes
        // (and then this "infects" the chrome statefile when you try and open a profile with it)
        let last_used_profile = local_statefile_obj["profile"]["last_used"].as_str().unwrap();
        return Some(String::from(last_used_profile));
    }

    pub fn get_profile_names(&self) -> Option<Vec<String>>
    {
        let local_statefile_obj = self.open_local_statefile_as_object();
        let mut profile_names: Vec<String> = Vec::new();

        let json_profile_names = &local_statefile_obj["profile"]["info_cache"];
        if json_profile_names.is_object()
        {
            for profile_name in json_profile_names.as_object().unwrap()
            {
                profile_names.push(String::from(profile_name.0.as_str()));
            }

            return Some(profile_names);
        }
        
        return None;
    }

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

}





/*
let img_bytes = reqwest::blocking::get("...")?
    .bytes()?;

let image = image::load_from_memory(&img_bytes)?;

"account_info": [
    {
      "account_id": "101562270597223606393",
      "accountcapabilities": {
        "accountcapabilities/g42tslldmfya": 1,
        "accountcapabilities/gi2tklldmfya": 1,
        "accountcapabilities/gu2dqlldmfya": 1,
        "accountcapabilities/gu4dmlldmfya": 0,
        "accountcapabilities/guydolldmfya": 0,
        "accountcapabilities/guzdslldmfya": 0,
        "accountcapabilities/haytqlldmfya": 1
      },
      "email": "krufty78@gmail.com",
      "full_name": "Garret Thomson",
      "gaia": "101562270597223606393",
      "given_name": "Garret",
      "hd": "NO_HOSTED_DOMAIN",
      "is_supervised_child": 0,
      "is_under_advanced_protection": false,
      "last_downloaded_image_url_with_size": "https://lh3.googleusercontent.com/a/AEdFTp7nyh4xYCDWmOGLz8eyQfciQxIVTfQMmAv4KaIOHl8=s256-c-ns",
      "locale": "en",
      "picture_url": "https://lh3.googleusercontent.com/a/AEdFTp7nyh4xYCDWmOGLz8eyQfciQxIVTfQMmAv4KaIOHl8=s96-c"
    }

 */


