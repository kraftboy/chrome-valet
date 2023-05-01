use registry::{Hive, RegKey, Security};
use std::fmt;
use utfx::U16CString;

pub struct BrowserDefinition {
    pub browser_exe: String,
    pub url_class_name: String,
    pub app_data_dir: String,
}

#[derive(PartialEq)]
pub enum Browser {
    Chrome,
    Brave,
    Unknown,
    // Edge,
    // Firefox,
}

impl fmt::Display for Browser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Browser::Chrome => write!(f, "chrome"),
            Browser::Brave => write!(f, "brave"),
            Browser::Unknown => write!(f, ""),
        }
    }
}

impl TryFrom<&String> for Browser {
    type Error = &'static str;

    fn try_from(value: &String) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "chrome" => Ok(Browser::Chrome),
            "brave" => Ok(Browser::Brave),
            "" => Ok(Browser::Unknown),
            _ => Err("error!"),
        }
    }
}

impl Browser {
    pub fn get_definition(&self) -> Option<BrowserDefinition> {
        match *self {
            Browser::Chrome => Some(BrowserDefinition { browser_exe: "chrome.exe".to_owned(), url_class_name: "ChromeHTML".to_owned(), app_data_dir: "Google\\Chrome".to_owned() }),
            Browser::Brave => Some(BrowserDefinition { browser_exe: "brave.exe".to_owned(), url_class_name: "BraveHTML".to_owned(), app_data_dir: "BraveSoftware\\Brave-Browser".to_owned() }),
            Browser::Unknown => None
            // Browser::Firefox => BrowserDefinition { browser_exe: "firefox.exe".to_owned(), url_class_name: "FirefoxHTML-[crazynumber]".to_owned(), app_data_dir: "BraveSoftware\Brave-Browser".to_owned() },
            // Browser::Edge => BrowserDefinition { browser_exe: "msedge.exe".to_owned(), url_class_name: "MSEdgeHTM".to_owned() },
        }
    }
}

pub fn get_browser_exe(browser_type: &Browser) -> Result<String, registry::Error> {
    let mut browser_exe = Browser::Chrome.get_definition().unwrap().browser_exe;
    if let Some(browser_def) = browser_type.get_definition() {
        browser_exe = browser_def.browser_exe;
    }

    let regkey: RegKey = Hive::LocalMachine.open(
        format!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths\\{browser_exe}"),
        Security::Read,
    )?;
    let value: String = match regkey.value(U16CString::default()) {
        Err(e) => return Err(registry::Error::Value(e)),
        Ok(v) => v.to_string(),
    };

    Ok(value)
}

#[allow(dead_code)]
pub fn get_browser_launch_command(browser_type: &Browser) -> Result<String, registry::Error> {
    let mut url_class_name = Browser::Chrome.get_definition().unwrap().url_class_name;
    if let Some(browser_def) = browser_type.get_definition() {
        url_class_name = browser_def.url_class_name;
    }

    let regkey: RegKey = Hive::LocalMachine.open(
        format!("Software\\Classes\\{url_class_name}\\shell\\open\\command"),
        Security::Read,
    )?;
    let value: String = match regkey.value(U16CString::default()) {
        Err(e) => return Err(registry::Error::Value(e)),
        Ok(v) => v.to_string(),
    };

    Ok(value)
}

pub fn is_default_browser() -> Result<bool, registry::Error> {
    let regkey = Hive::CurrentUser.open(
        r"Software\Microsoft\Windows\Shell\Associations\UrlAssociations\http\UserChoice",
        Security::Read,
    )?;
    match regkey.value(U16CString::from_os_str("ProgID").unwrap()) {
        Err(e) => Err(registry::Error::Value(e)),
        Ok(v) => Ok(v.to_string() == "ChromeValetURL"),
    }
}

pub fn get_default_browser() -> Result<Option<Browser>, registry::Error> {
    let regkey = Hive::CurrentUser.open(
        r"Software\Microsoft\Windows\Shell\Associations\UrlAssociations\http\UserChoice",
        Security::Read,
    )?;
    match regkey.value(U16CString::from_os_str("ProgID").unwrap()) {
        Err(e) => Err(registry::Error::Value(e)),
        Ok(v) => match v.to_string().as_str() {
            "ChromeHTML" => Ok(Some(Browser::Chrome)),
            "BraveHTML" => Ok(Some(Browser::Brave)),
            _ => Ok(None),
        },
    }
}
