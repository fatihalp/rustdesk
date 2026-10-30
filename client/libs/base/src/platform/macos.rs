use hbb_common::ResultType;
use osascript;
use serde_derive::{Deserialize, Serialize};

#[derive(Serialize)]
struct AlertParams {
    title: String,
    message: String,
    alert_type: String,
    buttons: Vec<String>,
}

#[derive(Deserialize)]
struct AlertResult {
    #[serde(rename = "buttonReturned")]
    button: String,
}

pub fn alert(
    app: String,
    alert_type: String,
    title: String,
    message: String,
    buttons: Vec<String>,
) -> ResultType<String> {
    let script = osascript::JavaScript::new(&format!(
        "
    var App = Application('{}');
    App.includeStandardAdditions = true;
    return App.displayAlert($params.title, {{
        message: $params.message,
        'as': $params.alert_type,
        buttons: $params.buttons,
    }});
    ",
        app
    ));

    let result: AlertResult = script.execute_with_params(AlertParams {
        title,
        message,
        alert_type,
        buttons,
    })?;
    Ok(result.button)
}
