// This file contains all structs and enums and associated implementations used in this app

pub mod data_forms {
    #![allow(non_snake_case)]

    use serde_derive::{Deserialize, Serialize};
    use std::collections::HashMap;
    use std::fmt;
    use sysinfo::{System, SystemExt};

    // Config structs
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct ConfigFile {
        pub log_folder: String,
        pub database_path: String,
        pub email_body_scheme: String,
    }
    impl ::std::default::Default for ConfigFile {
        fn default() -> Self {
            Self {
                log_folder: "./rsc/auditlogs/".to_string(),
                database_path: "./rsc/dmnb.sqlite".to_string(),
                email_body_scheme: "./rsc/email_body.html".to_string(),
            }
        }
    }
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct ConfigMain {
        pub file_locations: ConfigFile,
        pub smtp_config: ConfigSmtp,
    }
    impl ::std::default::Default for ConfigMain {
        fn default() -> Self {
            Self {
                file_locations: ConfigFile::default(),
                smtp_config: ConfigSmtp::default(),
            }
        }
    }
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct ConfigSmtp {
        pub admin_mail_addr: Option<String>,
        pub smtp_server: String,
        pub smtp_username: String,
        pub smtp_password: String,
    }
    impl ::std::default::Default for ConfigSmtp {
        fn default() -> Self {
            Self {
                admin_mail_addr: None,
                smtp_server: "127.0.0.1".to_string(),
                smtp_username: "test".to_string(),
                smtp_password: "test".to_string(),
            }
        }
    }

    // Request Payload
    #[derive(Serialize, Deserialize)]
    pub struct RequestPayload {
        pub T: Option<u32>,
        pub Tp: Option<u32>,
        pub Td: Option<u32>,
        pub L: Option<Vec<String>>,
        pub O: Option<HashMap<String, Vec<String>>>,
    }
    impl RequestPayload {
        /// Tries to create a Payload from a given string
        /// the string has to be encoded as JSON otherwise this function will panic!
        pub fn from_json(content: &String) -> Self {
            serde_json::from_str(content).unwrap()
        }
    }

    // HTTP Response
    #[derive(Serialize)]
    pub struct ResponsePayload {
        status: u16,
        content: ResponsePayloadTypes,
    }
    impl ResponsePayload {
        /// Create a new ResponsePayload with given Status code and given content
        pub fn new(status: u16, content: ResponsePayloadTypes) -> Self {
            Self { status, content }
        }
        /// Create a new ResponsePayload with given Status code and given Message string
        pub fn new_message(status: u16, message: String) -> Self {
            Self {
                status,
                content: ResponsePayloadTypes::Message(message),
            }
        }
        /// Create a new ResponsePayload with given Status code and given Message slice
        pub fn new_static_message(status: u16, message: &str) -> Self {
            Self {
                status,
                content: ResponsePayloadTypes::Message(message.to_string()),
            }
        }
        /// Create a new ResponsePayload with Status 200 and standardized Message
        pub fn status_200() -> Self {
            Self {
                status: 200,
                content: ResponsePayloadTypes::Message("Ok".to_string()),
            }
        }
        /// Create a new ResponsePayload with Status 400 and standardized message
        pub fn status_400() -> Self {
            Self {
                status: 400,
                content: ResponsePayloadTypes::Message("Bad Request".to_string()),
            }
        }
        /// Create a new ResponsePayload with Status 500 and standardized message
        pub fn status_500() -> Self {
            Self {
                status: 500,
                content: ResponsePayloadTypes::Message(
                    "Internal Server Error\nPlease try again later".to_string(),
                ),
            }
        }
    }
    #[derive(Serialize)]
    pub enum ResponsePayloadTypes {
        Message(String),
        User(User), // As defined in src/data_handler.rs
        Status(ServerStatus),
    }

    // Server Status
    #[derive(Serialize, Deserialize)]
    pub struct ServerStatus {
        Hostname: String,
        Description: String,
        Account: String,
        Uptime: u32,
        Maintenace: i64,
    }
    impl ServerStatus {
        pub fn new(
            Description: String,
            Account_Email: String,
            Uptime: u32,
            Maintenace: i64,
        ) -> Self {
            Self {
                Hostname: {
                    let s = System::new();
                    s.host_name().unwrap_or("".to_string())
                },
                Description,
                Account: Account_Email,
                Uptime,
                Maintenace,
            }
        }
    }

    /// The User Object, as it's displayed in the database.
    /// id: A unique identifier also used as the api-key or 'username'
    /// email: used for notification and sign up
    /// state: The state of the user: -1 Unknown, 0 Normal, 10 Deceased, 15  Deceased and Notified (aka. completed)
    #[derive(Serialize, PartialEq, Debug, Clone)]
    pub struct User {
        pub id: String,
        pub email: String,
        pub state: i8,
    }

    impl fmt::Display for User {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "({}: {})", self.id, self.email)
        }
    }
    impl User {
        /// Returns an Empty User with id 0 and empty email and state 10 (Deceased)
        /// (Test Users are ALWAYS deceased)
        pub fn empty() -> Self {
            Self {
                id: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
                email: "".to_string(),
                state: 10,
            } // An Empty or non-existent user / test user is ALWAYS deceased
        }
    }

    /// The Verification object, as it's displayed in the database.
    /// email: the email address of the account
    /// code: the verification code, a number of up to 18 Digits
    /// expires: a timestamp indicating when the verification code becomes invalid
    #[derive(PartialEq, Debug, Clone)]
    pub struct Verification {
        pub email: String,
        pub code: u64,
        pub expires: u32,
    }
    impl fmt::Display for Verification {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "({}: {} - {})", self.email, self.code, self.expires)
        }
    }
    impl Verification {
        /// returns an Empty Verification Entry with empty email, code 0 and expires on 0
        pub fn empty() -> Self {
            Self {
                email: "".to_string(),
                code: 000000000000000000, // 18 Digits long
                expires: 0,
            }
        }
    }
}
