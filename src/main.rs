mod request_handler;
pub use crate::request_handler::handler;

mod data_handler;
pub use crate::data_handler::sqlite_handler;

mod state_engine;
pub use crate::state_engine::state_functions;

use chrono::{self, Local};
use env_logger::Builder;
use log::LevelFilter;
use rand::Rng;
use std::{
    collections::HashMap,
    convert::TryInto,
    io::Write,
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

fn main() {
    // TODO: Read this path from a config file!
    let database_path = "./dmnb.sqlite";
    // Build Logger
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
        //.filter(None, LevelFilter::Info)
        .filter(None, LevelFilter::Debug)
        .init();

    log::info!("Started");

    // Spawn Thread to check whenever a message was expected and received, and delete outtimed verifications
    let (tx, rx): (Sender<(String, u32)>, Receiver<(String, u32)>) = mpsc::channel();
    thread::spawn(move || {
        // Get the database connection
        let db = sqlite_handler::DatabaseState::init(database_path.to_string())
            .expect("Failed to connect to database");
        let verify_db = sqlite_handler::DatabaseState::init_with_table_name(
            database_path.to_string(),
            "verification".to_string(),
        )
        .expect("Failed to connect to database");
        // Create tables if not already existent
        db.create_table_for_user().expect("Failed to create table for users");
        verify_db.create_table_for_verification().expect("Failed to create table for verification");

        let mut rng = rand::thread_rng();
        let mut alltimes: HashMap<String, u32> = HashMap::new();
        loop {
            // Users System
            loop {
                match rx.try_recv() {
                    Ok(val) => {
                        alltimes.insert(val.0, val.1);
                    }
                    Err(err) => match err {
                        mpsc::TryRecvError::Empty => break,
                        mpsc::TryRecvError::Disconnected => {
                            log::error!("{}", err);
                            break;
                        }
                    },
                };
            }
            let current_time: u32 = chrono::offset::Utc::now()
                .timestamp()
                .try_into()
                .expect("Time went backwards");
            for (id, time) in alltimes.clone().iter() {
                // Check if user is outtimed
                if &current_time > time {
                    // If yes, update the state of that user
                    if let Err(e) = db.update_state_user(&id.to_string(), 10) {
                        log::error!("Failed to update state of outtimed user!\n: {}", e);
                    };
                    alltimes.remove(id).unwrap();
                    // Log that the user has been set to deceased
                    if let Ok(Some(user)) = db.get_user_by_id(id) {
                        state_functions::custom_log_line(
                            &user,
                            "User was marked as deceased due to timeout".to_string(),
                        )
                        .unwrap();
                    }
                    log::debug!("USER {} just outtimed and was marked as `deceased`", id);
                }
            }
            // Verification System will only happen with a propability of 1:1000 to not overload the
            // database (might be wrong reasoning). In any case expiration should be checked when
            // the key is submitted by the user.
            if rng.gen_range(0..1000) == 1 {
                if let Err(e) = verify_db.delete_outtimed_verifications() {
                    log::error!("Failed to update verifications!\n: {}", e);
                }
            }
        }
    });

    handler::run(database_path.to_string(), tx).unwrap_or_else(|err| log::error!("{}", err));
}
