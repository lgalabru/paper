#![feature(plugin, decl_macro, proc_macro_hygiene)]

extern crate serde;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate rocket;

#[macro_use]
extern crate serde_json;

mod frontend;
mod generate;
mod integrate;
mod poke;
mod publish;
mod test;
mod types;
mod utils;

use frontend::cli;

pub fn main() {
    cli::main();
}
