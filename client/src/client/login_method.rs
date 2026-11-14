use super::LoginConfigHandler;

#[derive(Default, PartialEq, Eq)]
pub(super) enum LoginMethod {
    #[default]
    Automatic,
    RequestApproval,
}

impl LoginConfigHandler {
    pub(crate) fn set_login_method(&mut self, method: &str) -> bool {
        self.login_method = match method {
            "request-approval" => LoginMethod::RequestApproval,
            _ => return false,
        };
        true
    }

    pub(crate) fn skips_saved_password(&self) -> bool {
        self.login_method != LoginMethod::Automatic
    }

    pub(crate) fn get_login_method(&self) -> &'static str {
        match self.login_method {
            LoginMethod::Automatic => "",
            LoginMethod::RequestApproval => "request-approval",
        }
    }
}
