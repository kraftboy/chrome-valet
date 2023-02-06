// add registry keys to
// like Computer\HKEY_CLASSES_ROOT\BraveHTML



// change associations in 
// Computer\HKEY_CURRENT_USER\Software\Microsoft\Windows\Shell\Associations\UrlAssociations\http\UserChoice

use utfx::U16CString;
use registry::{RegKey, Hive, Data, Security};

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

pub fn create_registry_keys() -> Result<(), registry::Error>
{
    // https://stackoverflow.com/questions/50158532/registering-a-java-application-as-the-default-browser-in-windows-10

    let create_key = |name| -> Result<RegKey, registry::key::Error>
    {
        let key: RegKey;
        match Hive::CurrentUser.create(name, Security::Read | Security::Write)
        {
            Err(e) => {
                println!("Couldn't create ChromeProfilePickerHTML regkey: {}, {}", name, e);
                return Err(e);
            }
            Ok(x) => {
                key = x;
                println!("Key created: {}", name)
            }
        }

        Ok(key)
    };

    let create_value = |regkey: &RegKey, name: &str, val: &str| -> Result<(), registry::value::Error>
    {
        match regkey.set_value(U16CString::from_str(name).unwrap(), &Data::String(U16CString::from_str(val).unwrap())) {
            Err(e) => {
                println!("Couldn't delete key value: {}, {}", name, e);
                return Err(e);
            },
            _ => println!("Value created: {} {}", name, val)
        }

        Ok(())
    };

    // register client
    let mut regkey = create_key(r"SOFTWARE\Clients\StartMenuInternet\ChromeProfilePicker\Capabilities")?;
    create_value(&regkey, r"ApplicationDescription", r#"ChromeProfilePicker"#)?;
    create_value(&regkey, r"ApplicationIcon", r#"C:\Users\g\source\repos\chrome_profile_proxy\target\debug\chrome_profile_proxy.exe,0"#)?;
    create_value(&regkey, r"ApplicationName", r#"ChromeProfilePicker"#)?;

    regkey = create_key(r"SOFTWARE\Clients\StartMenuInternet\ChromeProfilePicker\Capabilities\URLAssociations")?;
    create_value(&regkey, r"http", r#"ChromeProfilePickerHTML"#)?;
    create_value(&regkey, r"https", r#"ChromeProfilePickerHTML"#)?;

    regkey = create_key(r"SOFTWARE\Clients\StartMenuInternet\ChromeProfilePicker\DefaultIcon")?;
    create_value(&regkey, r"", r#"C:\Users\g\source\repos\chrome_profile_proxy\target\debug\chrome_profile_proxy.exe,0"#)?;

    regkey = create_key(r"SOFTWARE\Clients\StartMenuInternet\ChromeProfilePicker\shell\open\command")?;
    create_value(&regkey, r"", r#"C:\Users\g\source\repos\chrome_profile_proxy\target\debug\chrome_profile_proxy.exe"#)?;

    
    // register url handler

    regkey = create_key(r"Software\Classes\ChromeProfilePickerHTML")?;
    create_value(&regkey, r"", r"ChromeProfilePicker HTML")?;
    regkey.set_value(U16CString::default(), &Data::U32(0x2))?;
    create_value(&regkey, r"FriendlyTypeName","MyApp Document")?;
    create_value(&regkey, r"URL Protocol", String::default().as_str())?;

    regkey = create_key(r"Software\Classes\ChromeProfilePickerHTML\DefaultIcon")?;
    create_value(&regkey, r"", r#"C:\Users\g\source\repos\chrome_profile_proxy\target\debug\chrome_profile_proxy.exe,0"#)?;


    /*
    // don't think I need this
    regkey = create_key(r"Software\Classes\ChromeProfilePickerHTML\shell")?;
    [HKEY_LOCAL_MACHINE\Software\Classes\ChromeProfilePickerHTML\shell]
    @='open'
    */

    regkey = create_key(r"Software\Classes\ChromeProfilePickerHTML\shell\open\command")?;
    create_value(&regkey, r"", r#"C:\Users\g\source\repos\chrome_profile_proxy\target\debug\chrome_profile_proxy.exe --url "%1""#)?;

    return Ok(())
}

pub fn delete_registry_keys() -> Result<(), registry::Error>
{
    let delete_key = |key| -> Result<(), registry::Error> {
        match Hive::CurrentUser.delete(key, true)
        {
            Err(e) => println!("Couldn't delete key: {}, {}", key, e),
            _ => ()
        };

        Ok(())
    };
    
    let keys_to_delete = [
        r"SOFTWARE\Clients\StartMenuInternet\ChromeProfilePicker\Capabilities",
        r"SOFTWARE\Clients\StartMenuInternet\ChromeProfilePicker\Capabilities\URLAssociations",
        r"SOFTWARE\Clients\StartMenuInternet\ChromeProfilePicker\DefaultIcon",
        r"SOFTWARE\Clients\StartMenuInternet\ChromeProfilePicker\shell\open\command",
        r"Software\Classes\ChromeProfilePickerHTML",
        r"Software\Classes\ChromeProfilePickerHTML\DefaultIcon",
        r"Software\Classes\ChromeProfilePickerHTML\shell\open\command",
    ];

    for key in keys_to_delete {
        delete_key(key)?;
    }

    Ok(())
}