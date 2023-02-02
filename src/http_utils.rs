pub fn escape_for_cmd(url: String) -> String
{
    let mut escaped_url = url;
    // for escaping the ampersands:
    // https://stackoverflow.com/questions/60880436/how-to-pass-parameter-with-ampersand-when-calling-a-batch-from-a-batch
    // need to escape: & , | , ( , ) , < , > , ^
    let chars_to_escape = ["&", "|", "(",")", "<", ">", "^"];
    for char in chars_to_escape
    {
        escaped_url = escaped_url.replace(char, format!("^{}", char).as_str());
    }

    return escaped_url;
}

