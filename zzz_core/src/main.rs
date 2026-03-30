use std::io::{Write};
use libc::exit;

mod model;
mod utils;
mod binary_data_process;
mod shell;



fn main() {
    use crate::shell::full_shell::Shell;
    let mut shell = Shell::new("sh").expect("create shell failed");
    use std::io;
    use colored::*;


    shell.on_output(move |line| {
        println!("\r{}", format!("{line}").green());
    });

    shell.on_error(move |line| {
        eprintln!("\r{}", format!("{line}").red());
    });

    shell.on_exit(move |code| {
        println!("{}", format!("[exit] code = {code}").blue());
        // unsafe { exit(code); }
    });

    fn read_line() -> String{
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .expect("failed to read line");
        input
    }

    loop {


        let input = read_line();

        match shell.send(&format!("{input}")) {
            Ok(_) => (),
            Err(e) => {
                eprintln!("{}", format!("{e}").red());
                unsafe { exit(-1); }
            },
        }
    }
}