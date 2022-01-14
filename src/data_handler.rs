pub mod sqlite_handler {
    use crate::data::data_forms::{User, Verification};
    use chrono::Utc;
    use rand::{distributions::Alphanumeric, Rng};
    use rusqlite::{self, Connection};
    use std::convert::TryFrom;

    #[derive(Debug)]
    pub struct DatabaseState {
        connection: Connection,
        table_name: String,
    }
    impl DatabaseState {
        /// Initialize a new database state
        pub fn init(db_path: String) -> std::result::Result<Self, rusqlite::Error> {
            let table_name = "users".to_string();
            let connection = Connection::open(db_path)?;
            Ok(Self {
                table_name,
                connection,
            })
        }
        /// Initialize a new database state with a given table name
        pub fn init_with_table_name(
            db_path: String,
            table_name: String,
        ) -> std::result::Result<Self, rusqlite::Error> {
            let connection = Connection::open(db_path)?;
            Ok(Self {
                table_name,
                connection,
            })
        }
        pub fn kill(self) -> std::result::Result<(), rusqlite::Error> {
            let mut conn = self.connection;
            let mut err = rusqlite::Error::InvalidQuery;
            for _ in 0..5 {
                let r = conn.close();
                match r {
                    Ok(_) => return Ok(()),
                    Err(e) => {
                        conn = e.0;
                        err = e.1;
                    }
                }
            }

            Err(err)
        }
        /// Create a new table for the User struct, if not already present
        pub fn create_table_for_user(&self) -> std::result::Result<(), rusqlite::Error> {
            self.connection.execute(
                    &format!(
                        "CREATE TABLE IF NOT EXISTS {} ('id' TEXT, 'email' TEXT, 'state' INTEGER, PRIMARY KEY('id'))",
                        self.table_name
                    ),
                    []
                )?;
            Ok(())
        }
        /// Create a new table for the Verification struct, if not already present
        pub fn create_table_for_verification(&self) -> std::result::Result<(), rusqlite::Error> {
            self.connection.execute(
                &format!(
                    "CREATE TABLE IF NOT EXISTS {} ('email' TEXT, 'code' INTEGER, 'expires' INTEGER)",
                    self.table_name
                ),
                [],
            )?;
            Ok(())
        }
        /// Delete Table if present
        pub fn delete_table(&self) -> std::result::Result<(), rusqlite::Error> {
            self.connection
                .execute(&format!("DROP TABLE IF EXISTS {}", self.table_name), [])?;
            Ok(())
        }
        /// So far useless (just as me)
        pub fn clean_table(&self) -> std::result::Result<(), rusqlite::Error> {
            Ok(())
        }
        /// Retrieves a User by its ID, returns the first found or None if none were found.
        pub fn get_user_by_id(
            &self,
            id: &String,
        ) -> std::result::Result<Option<User>, rusqlite::Error> {
            let mut q = self.connection.prepare(&format!(
                "SELECT id, email, state FROM {} WHERE id = (?)",
                self.table_name
            ))?;
            let mut results = q.query_map([id], |row| {
                Ok(User {
                    id: row.get(0)?,
                    email: row.get(1)?,
                    state: row.get(2)?,
                })
            })?;
            match results.next() {
                Some(val) => Ok(Some(val?)),
                None => Ok(None),
            }
        }
        /// Retrieves a User by its Email, returns the first found or None if none were found.
        pub fn get_user_by_email(
            &self,
            email: &String,
        ) -> std::result::Result<Option<User>, rusqlite::Error> {
            let mut q = self.connection.prepare(&format!(
                "SELECT id, email, state FROM {} WHERE email = (?)",
                self.table_name
            ))?;
            let mut results = q.query_map([email], |row| {
                Ok(User {
                    id: row.get(0)?,
                    email: row.get(1)?,
                    state: row.get(2)?,
                })
            })?;
            match results.next() {
                Some(val) => Ok(Some(val?)),
                None => Ok(None),
            }
        }
        /// Generates a new User-id and adds it with the given email to the database, then returns the full user
        /// The ID is generated randomly and regenerated if it already existed. If there are a lot of ids already,
        /// the process will take longer, note that this function has no timeout by itself.
        pub fn new_user(&self, email: &String) -> std::result::Result<User, rusqlite::Error> {
            log::debug!("Creating New User...");
            let new_id = loop {
                let gen_id: String = rand::thread_rng()
                    .sample_iter(&Alphanumeric)
                    .take(64)
                    .map(char::from)
                    .collect();

                let mut check_id = self.connection.prepare(&format!(
                    "SELECT * FROM {} WHERE id = '{}'",
                    self.table_name, gen_id
                ))?;
                let mut results = check_id.query_map([], |row| {
                    Ok(User {
                        id: row.get(0)?,
                        email: row.get(1)?,
                        state: row.get(2)?,
                    })
                })?;
                if match results.next() {
                    Some(_) => true,
                    None => false,
                } {
                    drop(results);
                    log::debug!("Generated ID already exists!, generating new one...");
                    check_id.finalize()?;
                    continue;
                } else {
                    log::debug!("Generated new ID successfully");
                    break gen_id;
                }
            };
            log::debug!("Writing changes to Database...");
            self.connection.execute(
                &format!(
                    "INSERT INTO {} (id, email, state) VALUES ((?), (?), (?))",
                    self.table_name
                ),
                [new_id.clone(), email.clone(), 0.to_string()],
            )?;

            log::debug!("Created a new User successfully");
            Ok(User {
                id: new_id,
                email: email.to_string(),
                state: 0,
            })
        }
        /// Deletes the given user from the database by it's id.
        pub fn delete_user(&self, user_id: &String) -> std::result::Result<(), rusqlite::Error> {
            self.connection.execute(
                &format!("DELETE FROM {} WHERE id = (?)", self.table_name),
                [user_id],
            )?;
            Ok(())
        }
        /// Update the state of an existing user by its ID.
        /// *This won't do anything if the user is already deceased!!*
        /// Returns true if updated successfully, passes errors and returns
        /// false if not updated (as user is already deceased)
        pub fn update_state_user(
            &self,
            id: &String,
            new_state: i8,
        ) -> std::result::Result<bool, rusqlite::Error> {
            if self.get_user_by_id(id)?.unwrap_or(User::empty()).state < 10 {
                self.connection.execute(
                    &format!("UPDATE {} SET state = (?) WHERE id = (?)", self.table_name),
                    [new_state.to_string(), id.to_string()],
                )?;
                Ok(true)
            } else {
                Ok(false)
            }
        }
        /// Select all users by a given state
        pub fn get_user_by_state(
            &self,
            state: i8,
        ) -> std::result::Result<Vec<User>, rusqlite::Error> {
            let mut q = self.connection.prepare(&format!(
                "SELECT * FROM {} WHERE state = (?)",
                self.table_name
            ))?;
            let results = q.query_map([state], |row| {
                Ok(User {
                    id: row.get(0)?,
                    email: row.get(1)?,
                    state: row.get(2)?,
                })
            })?;

            results.collect()
        }
        /// Generate a new verification entry, provide the email. Will return the generated
        /// Verification struct. The expiration time will be 10 minutes from now.
        /// If `duplicate_check` is set to `true` nothing will be done if the email already
        /// exists in the database
        pub fn generate_verification_code(
            &self,
            email: String,
            duplicate_check: bool,
        ) -> std::result::Result<Option<Verification>, rusqlite::Error> {
            if duplicate_check {
                // Do nothing if an email is found
                if self.get_verification_by_email(&email)?.is_some() {
                    return Ok(None);
                }
            }
            log::debug!("Generating a new verification entry");
            let code: u64 = loop {
                let gen_code: u64 = rand::thread_rng().gen_range(0..999999999999999999);
                let mut check_code = self.connection.prepare(&format!(
                    "SELECT * FROM {} WHERE code = '{}'",
                    self.table_name, gen_code
                ))?;
                let mut results = check_code.query_map([], |row| {
                    Ok(Verification {
                        email: row.get(0)?,
                        code: row.get(1)?,
                        expires: row.get(2)?,
                    })
                })?;
                if results.next().is_some() {
                    drop(results);
                    log::debug!(
                        "Generated verification code already exists!, generating new one..."
                    );
                    check_code.finalize()?;
                    continue;
                } else {
                    log::debug!("Generated new ID successfully");
                    break gen_code;
                }
            };

            let expires: u32 =
                u32::try_from(Utc::now().timestamp()).expect("Time went backwards") + 600;
            let r = Verification {
                email,
                code,
                expires,
            };
            log::debug!("Writing changes to Database...");
            self.connection.execute(
                &format!(
                    "INSERT INTO {} (email, code, expires) VALUES ((?), (?), (?))",
                    self.table_name
                ),
                [r.email.clone(), r.code.to_string(), r.expires.to_string()],
            )?;
            log::debug!("Created a new verification entry successfully");
            Ok(Some(r))
        }
        /// Select a verification entry by a given email
        pub fn get_verification_by_email(
            &self,
            email: &String,
        ) -> Result<Option<Verification>, rusqlite::Error> {
            let mut q = self.connection.prepare(&format!(
                "SELECT email, code, expires FROM {} WHERE email = (?)",
                self.table_name
            ))?;
            let mut results = q.query_map([email], |row| {
                Ok(Verification {
                    email: row.get(0)?,
                    code: row.get(1)?,
                    expires: row.get(2)?,
                })
            })?;
            match results.next() {
                Some(val) => Ok(Some(val?)),
                None => Ok(None),
            }
        }
        /// Verfies a given code. Returns None if the code is invalid. If the code is valid the
        /// Verification object is queried, removed from the database and the associated email is returned.
        pub fn verify_verification_code(
            self,
            code: u64,
        ) -> Result<Option<String>, rusqlite::Error> {
            let mut q = self.connection.prepare(&format!(
                "SELECT email, code, expires FROM {} WHERE code = (?)",
                self.table_name
            ))?;
            let mut results = q.query_map([code.to_string()], |row| {
                Ok(Verification {
                    email: row.get(0)?,
                    code: row.get(1)?,
                    expires: row.get(2)?,
                })
            })?;
            if let Some(entry) = results.next() {
                let entry = entry?;
                if entry.expires
                    < u32::try_from(Utc::now().timestamp()).expect("Time went backwards")
                {
                    return Ok(None);
                } else {
                    ()
                }
                self.connection.execute(
                    &format!("DELETE FROM {} WHERE code = (?)", self.table_name),
                    [entry.code],
                )?;
                Ok(Some(entry.email))
            } else {
                return Ok(None);
            }
        }
        /// Delete all verification entries which are expired
        pub fn delete_outtimed_verifications(&self) -> std::result::Result<(), rusqlite::Error> {
            self.connection.execute(
                &format!("DELETE FROM {} WHERE expires < (?)", self.table_name),
                [Utc::now().timestamp().to_string()],
            )?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::data::data_forms::User;
    use crate::sqlite_handler::DatabaseState;

    use chrono::Local;
    use env_logger::Builder;
    use log::LevelFilter;
    use std::io::Write;

    fn init_logging() {
        Builder::new()
            .format(|buf, record| {
                writeln!(
                    buf,
                    "[{}] {} - {}: {}",
                    record.level(),
                    Local::now().format("%d/%m/%y %H:%M:%S"),
                    record.target(),
                    record.args(),
                )
            })
            .filter(None, LevelFilter::Debug)
            .init();
    }

    #[test]
    fn create_new_user() {
        //init_logging();

        log::debug!("Testing New User Creation");
        let db = DatabaseState::init("./dmnb.sqlite".to_string()).unwrap();
        let user = db.new_user(&"foo@example.com".to_string()).unwrap();
        assert_eq!(user.email, "foo@example.com".to_string())
    }
    #[test]
    fn basic_table_operations() {
        init_logging();
        log::debug!("Creating new table called test for tesiting operation");
        let db =
            DatabaseState::init_with_table_name("./dmnb.sqlite".to_string(), "test".to_string())
                .unwrap();
        db.create_table_for_user().unwrap();

        log::debug!("Creating new&empty user for testing");
        let test_user = db.new_user(&"foobar@example.com".to_string()).unwrap();
        let empty_user = User::empty();
        log::debug!("Fetching new User by Id and Email");
        assert_eq!(
            match db.get_user_by_id(&test_user.id).unwrap() {
                Some(user) => user,
                None => panic!("No User has been found, while same user was just created"),
            },
            test_user
        );
        assert_eq!(
            match db.get_user_by_email(&test_user.email).unwrap() {
                Some(user) => user,
                None => panic!("No User has been found, while same user was just created"),
            },
            test_user
        );
        log::debug!("Fetching empty User by Id and Email");
        assert_eq!(
            match db.get_user_by_id(&empty_user.id).unwrap() {
                Some(_) => panic!("A User has been found which shouldn't have happened"),
                None => 0,
            },
            0
        );
        assert_eq!(
            match db.get_user_by_email(&empty_user.id).unwrap() {
                Some(_) => panic!("A User has been found which shouldn't have happened"),
                None => 0,
            },
            0
        );
        log::debug!("Updating State of the new User");
        assert_eq!(db.update_state_user(&test_user.id, 2), Ok(true));
        let upd_user = User {
            id: test_user.id.clone(),
            email: test_user.email.clone(),
            state: 2,
        };
        log::debug!("Get Users by state 2 and -1");
        assert_eq!(db.get_user_by_state(2), Ok(vec![upd_user]));
        assert_eq!(db.get_user_by_state(-1), Ok(vec![]));

        log::debug!("Deleting the new User");
        db.delete_user(&test_user.id).unwrap();
        assert_eq!(
            match db.get_user_by_id(&test_user.id).unwrap() {
                Some(_) =>
                    panic!("A User has been found although samer user has just been deleted"),
                None => 0,
            },
            0
        );
        log::debug!("Deleting test Table");
        db.create_table_for_user().unwrap();
        db.delete_table().unwrap();
        db.delete_table().unwrap();
    }
}
