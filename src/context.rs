use crate::messages::AuthenticationMethod;

pub struct Credentials {
    username: String,
    password: String,
}

impl Credentials {
    pub fn new(username: &str, password: &str) -> Self {
        Credentials {
            username: username.into(),
            password: password.into(),
        }
    }
}

#[derive(Default)]
pub struct Context {
    credentials: Option<Credentials>,
}

impl Context {
    pub fn with_credentials(credentials: Credentials) -> Self {
        Context {
            credentials: Some(credentials),
        }
    }

    pub fn select_authentication(
        &self,
        methods: Vec<AuthenticationMethod>,
    ) -> Option<AuthenticationMethod> {
        let expected_method = match self.credentials {
            Some(_) => AuthenticationMethod::UsernamePassword,
            None => AuthenticationMethod::NoAuthentication,
        };
        if methods.contains(&expected_method) {
            return Some(expected_method);
        }
        None
    }

    pub fn authenticate(&self, username: &str, password: &str) -> bool {
        match &self.credentials {
            Some(credentials) => {
                credentials.username == username && credentials.password == password
            }
            None => true,
        }
    }
}
