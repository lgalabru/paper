mod events_observer;
mod orchestrator;
mod ui;

use std::sync::mpsc::channel;

use chrono::prelude::*;

use crate::utils;
use events_observer::start_events_observer;
pub use orchestrator::DevnetOrchestrator;

use self::events_observer::{EventObserverConfig, PoxInfo};

pub fn run_devnet(devnet: DevnetOrchestrator) {
    match block_on(do_run_devnet(devnet)) {
        Err(_e) => std::process::exit(1),
        _ => {}
    };
}

pub fn block_on<F, R>(future: F) -> R
where
    F: std::future::Future<Output = R>,
{
    let rt = utils::create_basic_runtime();
    rt.block_on(future)
}

pub async fn do_run_devnet(mut devnet: DevnetOrchestrator) -> Result<bool, String> {
    let (devnet_events_tx, devnet_events_rx) = channel();
    let (termination_success_tx, orchestrator_terminated_rx) = channel();

    devnet.termination_success_tx = Some(termination_success_tx);

    let (devnet_config, accounts) = match devnet.network_config {
        Some(ref network_config) => match &network_config.devnet {
            Some(devnet_config) => Ok((devnet_config.clone(), network_config.accounts.clone())),
            _ => Err("Unable to retrieve config"),
        },
        _ => Err("Unable to retrieve config"),
    }?;

    // The event observer should be able to send some events to the UI thread,
    // and should be able to be terminated
    let config = EventObserverConfig {
        devnet_config,
        accounts,
        manifest_path: devnet.manifest_path.clone(),
        pox_info: PoxInfo::default(),
    };
    let events_observer_tx = devnet_events_tx.clone();
    let (events_observer_terminator_tx, terminator_rx) = channel();
    let events_observer_handle = std::thread::spawn(move || {
        let future = start_events_observer(config, events_observer_tx, terminator_rx);
        let rt = utils::create_basic_runtime();
        let _ = rt.block_on(future);
    });

    // The devnet orchestrator should be able to send some events to the UI thread,
    // and should be able to be restarted/terminated
    let orchestrator_event_tx = devnet_events_tx.clone();
    let (orchestrator_terminator_tx, terminator_rx) = channel();
    let orchestrator_handle = std::thread::spawn(move || {
        let future = devnet.start(orchestrator_event_tx, terminator_rx);
        let rt = utils::create_basic_runtime();
        rt.block_on(future);
    });

    let _ = ui::start_ui(
        devnet_events_tx,
        devnet_events_rx,
        events_observer_terminator_tx,
        orchestrator_terminator_tx,
        orchestrator_terminated_rx,
    );

    events_observer_handle.join().unwrap();
    orchestrator_handle.join().unwrap();

    Ok(true)
}

pub enum DevnetEvent {
    Log(LogData),
    KeyEvent(crossterm::event::KeyEvent),
    Tick,
    ServiceStatus(ServiceStatusData),
    Block(BlockData),
    MempoolAdmission(MempoolAdmissionData),
    // Restart,
    // Terminate,
    // Microblock(MicroblockData),
}

impl DevnetEvent {
    pub fn error(message: String) -> DevnetEvent {
        DevnetEvent::Log(Self::log_error(message))
    }

    #[allow(dead_code)]
    pub fn warning(message: String) -> DevnetEvent {
        DevnetEvent::Log(Self::log_warning(message))
    }

    pub fn info(message: String) -> DevnetEvent {
        DevnetEvent::Log(Self::log_info(message))
    }

    pub fn success(message: String) -> DevnetEvent {
        DevnetEvent::Log(Self::log_success(message))
    }

    pub fn debug(message: String) -> DevnetEvent {
        DevnetEvent::Log(Self::log_debug(message))
    }

    pub fn log_error(message: String) -> LogData {
        LogData::new(LogLevel::Error, message)
    }

    pub fn log_warning(message: String) -> LogData {
        LogData::new(LogLevel::Warning, message)
    }

    pub fn log_info(message: String) -> LogData {
        LogData::new(LogLevel::Info, message)
    }

    pub fn log_success(message: String) -> LogData {
        LogData::new(LogLevel::Success, message)
    }

    pub fn log_debug(message: String) -> LogData {
        LogData::new(LogLevel::Debug, message)
    }
}

pub enum LogLevel {
    Error,
    Warning,
    Info,
    Success,
    Debug,
}

pub struct LogData {
    pub occurred_at: String,
    pub message: String,
    pub level: LogLevel,
}

impl LogData {
    pub fn new(level: LogLevel, message: String) -> LogData {
        let now: DateTime<Utc> = Utc::now();
        LogData {
            level,
            message,
            occurred_at: now.format("%b %e %T%.6f").to_string(),
        }
    }
}

pub enum Status {
    Red,
    Yellow,
    Green,
}

pub struct ServiceStatusData {
    pub order: usize,
    pub status: Status,
    pub name: String,
    pub comment: String,
}

#[derive(Clone)]
pub struct Transaction {
    pub txid: String,
    pub success: bool,
    pub result: String,
    pub events: Vec<String>,
}

#[derive(Clone)]
pub struct BlockData {
    pub block_height: u32,
    pub block_hash: String,
    pub bitcoin_block_height: u32,
    pub bitcoin_block_hash: String,
    pub first_burnchain_block_height: u32,
    pub pox_cycle_length: u32,
    pub pox_cycle_id: u32,
    pub transactions: Vec<Transaction>,
}

// pub struct MicroblockData {
//     pub seq: u32,
//     pub transactions: Vec<Transaction>
// }

pub struct MempoolAdmissionData {
    pub txid: String,
}
