use utfx::U16CString;
use registry::{RegKey, Hive, Security};

pub fn get_chrome_exe() -> Result<String, registry::Error>
{
    let value: String;

    let regkey: RegKey = Hive::LocalMachine.open(r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\chrome.exe", Security::Read)?;
    match regkey.value(U16CString::default()) {
        Err(e) => return Err(registry::Error::Value(e)),
        Ok(v) => value = v.to_string(),
    }

    return Ok(value);
}