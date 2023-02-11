use utfx::U16CString;
use registry::{RegKey, Hive, Data, Security};
use log::{debug, error};

const APP_LOCATION: &str = r#"C:\Users\g\source\repos\chrome_profile_proxy\target\release\chrome_profile_proxy.exe"#;
const APP_LOCATION_ICON: &str = r#"C:\Users\g\source\repos\chrome_profile_proxy\target\release\chrome_profile_proxy.exe,0"#;
const APPLICATION_NAME: &str = "Chrome Valet";
const APPLICATION_REGISTRY_NAME: &str = "ChromeValet";

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

pub fn edit_registry_keys(create: bool) -> Result<(), registry::Error>
{
    // https://stackoverflow.com/questions/50158532/registering-a-java-application-as-the-default-browser-in-windows-10

    let mut keys = Vec::new();

    let mut create_key = |name: &String| -> Result<RegKey, registry::key::Error>
    {
        let key: RegKey;
        match Hive::CurrentUser.create(name, Security::Read | Security::Write)
        {
            Err(e) => {
                error!("Couldn't create regkey: {}, {}", name, e);
                return Err(e);
            }
            Ok(x) => {
                key = x;
                keys.push(name.clone());
                debug!("Key created: {}", name)
            }
        }

        Ok(key)
    };

    let create_value = |regkey: &RegKey, name: &str, val: &str| -> Result<(), registry::value::Error>
    {
        if !create {
            return Ok(());
        }

        match regkey.set_value(U16CString::from_str(name).unwrap(), &Data::String(U16CString::from_str(val).unwrap())) {
            Err(e) => {
                error!("Couldn't delete key value: {}, {}", name, e);
                return Err(e);
            },
            _ => debug!("\tValue created: {} {}", name, val)
        }

        Ok(())
    };

    // register client
    let mut key_name = "SOFTWARE\\Clients\\StartMenuInternet\\".to_owned() + APPLICATION_REGISTRY_NAME + "\\Capabilities";
    let mut regkey = create_key(&key_name)?;
    create_value(&regkey, r"ApplicationDescription", APPLICATION_REGISTRY_NAME)?;
    create_value(&regkey, r"ApplicationIcon", APP_LOCATION_ICON)?;
    create_value(&regkey, r"ApplicationName", APPLICATION_NAME)?;

    key_name = "SOFTWARE\\Clients\\StartMenuInternet\\".to_owned() + APPLICATION_REGISTRY_NAME + "\\Capabilities\\URLAssociations";
    regkey = create_key(&key_name)?;

    let mut value_name  = APPLICATION_REGISTRY_NAME.to_owned() + "URL";
    create_value(&regkey, r"http", &value_name)?;
    create_value(&regkey, r"https", &value_name)?;

    key_name = "SOFTWARE\\Clients\\StartMenuInternet\\".to_owned() + APPLICATION_REGISTRY_NAME + "\\DefaultIcon";
    regkey = create_key(&key_name)?;
    create_value(&regkey, r"", APP_LOCATION_ICON)?;

    key_name = "SOFTWARE\\Clients\\StartMenuInternet\\".to_owned() + APPLICATION_REGISTRY_NAME + "\\shell\\open\\command";
    regkey = create_key(&key_name)?;
    create_value(&regkey, r"", APP_LOCATION)?;

    
    // register url handler
    key_name = "SOFTWARE\\classes\\".to_owned() + APPLICATION_REGISTRY_NAME + "URL";
    regkey = create_key(&key_name)?;
    value_name = APPLICATION_REGISTRY_NAME.to_owned() + " URL";
    create_value(&regkey, r"", &value_name)?;
    regkey.set_value(U16CString::default(), &Data::U32(0x2))?;

    create_value(&regkey, r"FriendlyTypeName",&value_name)?;
    create_value(&regkey, r"URL Protocol", String::default().as_str())?;

    key_name = "Software\\Classes\\".to_owned() + APPLICATION_REGISTRY_NAME + "URL\\DefaultIcon";
    regkey = create_key(&key_name)?;
    create_value(&regkey, r"", APP_LOCATION_ICON)?;

    key_name = "Software\\Classes\\".to_owned() + APPLICATION_REGISTRY_NAME + "URL\\shell\\open\\command";
    regkey = create_key(&key_name)?;
    create_value(&regkey, r"", &(APP_LOCATION.to_string() + &r#" --url "%1""#.to_string()))?;

    key_name = "SOFTWARE\\RegisteredApplications".to_string();
    regkey = create_key(&key_name)?;
    value_name = "Software\\Clients\\StartMenuInternet\\".to_owned() + APPLICATION_REGISTRY_NAME + "\\Capabilities";
    create_value(&regkey, APPLICATION_REGISTRY_NAME, &value_name)?;
    
    if !create {
        for key_name in &keys
        {
            match Hive::CurrentUser.delete(key_name, true)
            {
                Err(e) =>  error!("Couldn't delete key: {}, {}", key_name, e),
                Ok(_) => debug!("Key deleted: {}", key_name),
            };    
        }
    }
    return Ok(())
}