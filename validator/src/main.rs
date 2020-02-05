use crate::validator::Config;
use crate::validator::Validator;
use clap::{App, Arg, ArgMatches, SubCommand};
use log::*;
use toml;

pub mod built_info;
mod commands;
mod config;
mod network;
mod services;
mod validator;

fn main() {
    dotenv::dotenv().ok();
    pretty_env_logger::init();

    println!("{}", banner());

    let arg_matches = App::new("Nym Validator")
        .version(built_info::PKG_VERSION)
        .author("Nymtech")
        .about("Implementation of Nym Validator")
        .subcommand(commands::init::command_args())
        .subcommand(commands::run::command_args())
        .get_matches();

    execute(arg_matches);
}

fn execute(matches: ArgMatches) {
    match matches.subcommand() {
        ("init", Some(m)) => commands::init::execute(m),
        ("run", Some(m)) => commands::run::execute(m),
        _ => println!("{}", usage()),
    }
}

fn usage() -> String {
    banner() + "usage: --help to see available options.\n\n"
}

fn banner() -> String {
    format!(
        r#"

      _ __  _   _ _ __ ___
     | '_ \| | | | '_ \ _ \
     | | | | |_| | | | | | |
     |_| |_|\__, |_| |_| |_|
            |___/

             (validator - version {:})

    "#,
        built_info::PKG_VERSION
    )
}
