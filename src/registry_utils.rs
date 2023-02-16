use utfx::U16CString;
use registry::{RegKey, Hive, Security};

pub fn get_chrome_exe() -> Result<String, registry::Error>
{
    

    let regkey: RegKey = Hive::LocalMachine.open(r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\chrome.exe", Security::Read)?;
    let value: String = match regkey.value(U16CString::default()) {
        Err(e) => return Err(registry::Error::Value(e)),
        Ok(v) => v.to_string(),
    };

    Ok(value)
}

pub fn is_default_browser() -> Result<bool, registry::Error>
{
    let regkey = Hive::CurrentUser.open(r"Software\Microsoft\Windows\Shell\Associations\UrlAssociations\http\UserChoice", Security::Read)?;
    match regkey.value(U16CString::from_os_str("ProgID").unwrap())
    {
        Err(e) => Err(registry::Error::Value(e)),
        Ok(v) => Ok(v.to_string() == "ChromeValetURL"),
    }
}