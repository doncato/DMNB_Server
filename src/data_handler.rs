pub mod sqlite_handler {
    use std::fmt;
    use rand::{Rng, distributions::Alphanumeric,};
    use rusqlite::{self, Connection,};

    #[derive(PartialEq, Debug, Clone,)]
    pub struct User {
        pub id: String,
        pub email: String,
    }
    impl fmt::Display for User {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "({}: {})", self.id, self.email)
        }
    }
    impl User {
        /// Returns an Empty User with id 0 and empty email
        pub fn empty() -> User {
            User { id: "000000000000000000000000000000000000".to_string(), email: "".to_string()}
        }
        /// Retrieves a User by its ID, returns the first found or None if none were found.
        pub fn get_by_id(db: &DatabaseState, id: String) -> std::result::Result<Option<User>, rusqlite::Error> {
            let mut q = db.connection.prepare(&format!("SELECT * FROM {} WHERE id = (?)", db.table_name))?;
            let mut results = q.query_map([id], |row| {
                Ok(User {
                    id: row.get(0)?,
                    email: row.get(1)?,
                })
            })?;
            match results.next() {
                Some(val) => Ok(Some(val?)),
                None => Ok(None),
            }
        }
        /// Retrieves a User by its Email, returns the first found or None if none were found.
        pub fn get_by_email(db: &DatabaseState, email: String) -> std::result::Result<Option<User>, rusqlite::Error> {
            let mut q = db.connection.prepare(&format!("SELECT * FROM {} WHERE email = (?)", db.table_name))?;
            let mut results = q.query_map([email], |row| {
                Ok(User {
                    id: row.get(0)?,
                    email: row.get(1)?,
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
        pub fn new_one(db: &DatabaseState, email: String) -> std::result::Result<User, rusqlite::Error> {
            log::debug!("Creating New User...");
            let new_id = loop {
                let gen_id: String = rand::thread_rng()
                    .sample_iter(&Alphanumeric)
                    .take(36)
                    .map(char::from)
                    .collect();

                //
                // I HATE THIS SOLUTIONS
                // I HAVE NO BETTER SOLUTION! IF YOU READ THIS, PLEASE! PLEAASE CHANGE THIS
                // ALL I FUCKING NEED TO DO IS KNOW WHETHER OR NOT _ANY_ ROWS WERE FOUND
                let mut check_id = db.connection.prepare(&format!("SELECT * FROM {} WHERE id = '{}'", db.table_name, gen_id))?;
                let mut results = check_id.query_map([], |row| {
                    Ok(User {
                        id: row.get(0)?,
                        email: row.get(1)?,
                    })
                })?;
                if match results.next() {
                    Some(_) => true,
                    None => false,
                } {
                    drop(results);
                    log::debug!("Generated ID already exists!, generating new one...");
                    check_id.finalize()?;
                    continue
                } else {
                    log::debug!("Generated new ID successfully");
                    break gen_id;
                }
            };
            log::debug!("Writing changes to Database...");
            db.connection
                .execute(
                    &format!("INSERT INTO {} (id, email) VALUES ((?), (?))", db.table_name),
                    [new_id.clone(), email.clone()]
                )?;

            log::debug!("Created a new User successfully");
            Ok(User {id: new_id, email})
        }
        /// Deletes the given user from the database by it's id.
        pub fn delete(self, db: &DatabaseState) -> std::result::Result<(), rusqlite::Error> {
            db.connection
                .execute(&format!("DELETE FROM {} WHERE id = (?)", db.table_name),
                [self.id]
            )?;
            Ok(())
        }
    }

    #[derive(Debug)]
    pub struct DatabaseState {
        connection: Connection,
        table_name: String,
    }
    impl DatabaseState {
        /// Initialize a new database state
        pub fn init(db_path: String) -> std::result::Result<DatabaseState, rusqlite::Error> {
            let table_name = "users".to_string();
            let connection = Connection::open(db_path)?;
            Ok(DatabaseState { table_name, connection })
        }
        /// Initialize a new database state with a given table name
        pub fn init_with_table_name(db_path: String, table_name: String) -> std::result::Result<DatabaseState, rusqlite::Error> {
            let connection = Connection::open(db_path)?;
            Ok(DatabaseState { table_name, connection })
        }
        pub fn kill(self) -> std::result::Result<(), rusqlite::Error> {
            let mut conn = self.connection;
            let mut err = rusqlite::Error::InvalidQuery;
            for _ in 0..5 {
                let r = conn.close();
                match r {
                    Ok(_) => {
                        return Ok(())
                    },
                    Err(e) => {
                        conn = e.0;
                        err = e.1;
                    },
                }
            }

            Err(err)
        }
        /// Create a new table if not already present
        pub fn create_table(&self) -> std::result::Result<(), rusqlite::Error> {
            self.connection
                .execute(
                    &format!("CREATE TABLE IF NOT EXISTS {} ('id' TEXT, 'email' TEXT, PRIMARY KEY('id'))", self.table_name),
                    []
                )?;
            Ok(())
        }
        pub fn delete_table(&self) -> std::result::Result<(), rusqlite::Error> {
            self.connection
                .execute(
                    &format!("DROP TABLE IF EXISTS {}", self.table_name),
                    []
                )?;
            Ok(())
        }
        pub fn clean_table(&self) -> std::result::Result<(), rusqlite::Error> {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::sqlite_handler::{
        DatabaseState,
        User,
    };

    use std::{io::Write,};
    use chrono::Local;
    use log::LevelFilter;
    use env_logger::Builder;

    fn init_logging() {
        Builder::new()
            .format(|buf, record| {
                writeln!(buf,
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
        init_logging();

        log::debug!("Testing New User Creation");
        let db = DatabaseState::init("/extern/prog/rust/dmnb_server_relais/dmnb.sqlite".to_string()).unwrap();
        let user = User::new_one(&db, "foo@example.com".to_string()).unwrap();
        assert_eq!(user.email, "foo@example.com".to_string())
    }
    #[test]
    fn basic_table_operations() {
        init_logging();
        log::debug!("Creating new table called test for tesiting operation");
        let db = DatabaseState::init_with_table_name("/extern/prog/rust/dmnb_server_relais/dmnb.sqlite".to_string(), "test".to_string()).unwrap();
        db.create_table().unwrap();

        log::debug!("Creating new&empty user for testing");
        let test_user = User::new_one(&db, "foobar@example.com".to_string()).unwrap();
        let empty_user = User::empty();
        log::debug!("Fetching new User by Id and Email");
        assert_eq!(
            match User::get_by_id(&db, test_user.clone().id).unwrap() {
                Some(user) => user,
                None => panic!("No User has been found, while same user was just created"),
            },
            test_user
        );
        assert_eq!(
            match User::get_by_email(&db, test_user.clone().email).unwrap() {
                Some(user) => user,
                None => panic!("No User has been found, while same user was just created"),
            },
            test_user
        );
        log::debug!("Fetching empty User by Id and Email");
        assert_eq!(
            match User::get_by_id(&db, empty_user.clone().id).unwrap() {
                Some(_) => panic!("A User has been found which shouldn't have happened"),
                None => 0,
            },
            0
        );
        assert_eq!(
            match User::get_by_email(&db, empty_user.clone().id).unwrap() {
                Some(_) => panic!("A User has been found which shouldn't have happened"),
                None => 0,
            },
            0
        );
        log::debug!("Deleting the new User");
        test_user.clone().delete(&db).unwrap();
        assert_eq!(
            match User::get_by_id(&db, test_user.id).unwrap() {
                Some(_) => panic!("A User has been found although samer user has just been deleted"),
                None => 0,
            },
            0
        );
        log::debug!("Deleting test Table");
        db.create_table().unwrap();
        db.delete_table().unwrap();
        db.delete_table().unwrap();
    }
}
