mod request_handler;
pub use crate::request_handler::handler;

mod data_handler;
pub use crate::data_handler::sqlite_handler;

mod state_engine;
pub use crate::state_engine::state_functions;

use std::{
    io::Write,
};
use chrono::Local;
use log::LevelFilter;
use env_logger::Builder;

fn main() {
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
        .filter(None, LevelFilter::Info)
        .init();

    log::info!("Started");

    handler::run().unwrap();
}

