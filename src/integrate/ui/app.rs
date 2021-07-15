use crate::integrate::{BlockData, LogData, MempoolAdmissionData, ServiceStatusData, Transaction};

use super::util::{StatefulList, TabsState};
use tui::style::{Color, Style};
use tui::text::{Span, Spans};

pub struct App<'a> {
    pub title: &'a str,
    pub should_quit: bool,
    pub blocks: Vec<BlockData>,
    pub tabs: TabsState<'a>,
    pub transactions: StatefulList<Transaction>,
    pub mempool: StatefulList<MempoolAdmissionData>,
    pub logs: StatefulList<LogData>,
    pub services: StatefulList<ServiceStatusData>,
}

impl<'a> App<'a> {
    pub fn new(title: &'a str) -> App<'a> {
        App {
            title,
            should_quit: false,
            tabs: TabsState::new(),
            blocks: vec![],
            transactions: StatefulList::with_items(vec![]),
            mempool: StatefulList::with_items(vec![]),
            logs: StatefulList::with_items(vec![]),
            services: StatefulList::with_items(vec![]),
        }
    }

    pub fn on_up(&mut self) {
        self.transactions.previous();
    }

    pub fn on_down(&mut self) {
        self.transactions.next();
    }

    pub fn on_right(&mut self) {
        self.tabs.next();
    }

    pub fn on_left(&mut self) {
        self.tabs.previous();
    }

    pub fn on_key(&mut self, c: char) {
        match c {
            'q' => {
                self.should_quit = true;
            }
            _ => {}
        }
    }

    pub fn on_tick(&mut self) {}

    pub fn display_service_status_update(&mut self, service_update: ServiceStatusData) {
        let insertion_index = service_update.order;
        if insertion_index == self.services.items.len() {
            self.services.items.push(service_update)
        } else {
            self.services.items.remove(insertion_index);
            self.services.items.insert(insertion_index, service_update)
        }
    }

    pub fn display_log(&mut self, log: LogData) {
        self.logs.items.push(log);
    }

    pub fn update_mempool(&mut self, tx: MempoolAdmissionData) {
        self.mempool.items.push(tx);
    }

    pub fn display_block(&mut self, block: BlockData) {
        let cycle_len = block.pox_cycle_length;
        let abs_pos = block.bitcoin_block_height - block.first_burnchain_block_height;
        let (start, end) = if abs_pos % cycle_len == (cycle_len - 1) {
            ("", "<")
        } else if abs_pos % cycle_len == 0 {
            (">", "")
        } else {
            ("", "")
        };
        let has_tx = if block.transactions.len() <= 1 {
            ""
        } else {
            "␂"
        };
        self.tabs.titles.push_front(Spans::from(Span::styled(
            format!("{}[{}{}]{}", end, block.block_height, has_tx, start),
            if block.pox_cycle_id % 2 == 1 {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::LightYellow)
            },
        )));
        self.blocks.push(block);
        if self.tabs.index != 0 {
            self.tabs.index += 1;
        }
    }
}
