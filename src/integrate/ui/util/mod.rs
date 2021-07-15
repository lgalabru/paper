use std::collections::VecDeque;

use tui::text::Spans;
use tui::widgets::ListState;

#[derive(Clone)]

pub struct TabsState<'a> {
    pub titles: VecDeque<Spans<'a>>,
    pub index: usize,
}

impl<'a> TabsState<'a> {
    pub fn new() -> TabsState<'a> {
        TabsState {
            titles: VecDeque::new(),
            index: 0,
        }
    }
    pub fn next(&mut self) {
        if self.titles.len() > 0 {
            self.index = (self.index + 1) % self.titles.len();
        }
    }

    pub fn previous(&mut self) {
        if self.titles.len() > 0 {
            if self.index > 0 {
                self.index -= 1;
            } else {
                self.index = self.titles.len() - 1;
            }
        }
    }
}

pub struct StatefulList<T> {
    pub state: ListState,
    pub items: Vec<T>,
}

impl<T> StatefulList<T> {
    pub fn new() -> StatefulList<T> {
        StatefulList {
            state: ListState::default(),
            items: Vec::new(),
        }
    }

    pub fn with_items(items: Vec<T>) -> StatefulList<T> {
        StatefulList {
            state: ListState::default(),
            items,
        }
    }

    pub fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if self.items.len() == 0 {
                    0
                } else if i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if self.items.len() == 0 {
                    0
                } else if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn unselect(&mut self) {
        self.state.select(None);
    }
}
