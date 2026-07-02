mod bytecode;
mod compiler;
mod vm;
mod environment;
mod expr;
mod interpreter;
mod parser;
mod resolver;
mod scanner;
mod stmt;
mod tests;
use crate::interpreter::*;
use crate::parser::*;
use crate::resolver::*;
use crate::scanner::*;
use crate::expr::{LiteralValue, CallableImpl};
use crate::vm::PyxisVM;
use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::process::exit;

#[allow(non_snake_case)]
fn isProductionMode() -> bool {
	env::var("PYXIS_ENV").map(|v| v.eq_ignore_ascii_case("production")).unwrap_or(false)
}

#[allow(non_snake_case)]
pub fn runStringOnVM(contents: &str) -> Result<(), String> {
	let mut vm = PyxisVM::new();
	vm.load(contents)
}

#[allow(non_snake_case)]
pub fn runFileOnVM(path: &str) -> Result<(), String> {
	let contents = fs::read_to_string(path).map_err(|e| e.to_string())?;
	let mut vm = PyxisVM::new();
	vm.load(&contents)
}

#[allow(non_snake_case)]
pub fn runFile(path: &str) -> Result<(), String> {
    // let mut interpreter = Interpreter::new();
    match fs::read_to_string(path) {
        Err(msg) => return Err(msg.to_string()),
        Ok(contents) => return runString(&contents),
    }
}

#[allow(non_snake_case)]
pub fn runString(contents: &str) -> Result<(), String> {
    let mut interpreter = Interpreter::new(isProductionMode());

    run(&mut interpreter, contents)
}

#[allow(non_snake_case)]
fn run(interpreter: &mut Interpreter, contents: &str) -> Result<(), String> {
    let mut scanner = Scanner::new(contents);
    let tokens = scanner.scanTokens()?;

    let mut parser = Parser::new(tokens);
    let stmts = parser.parse()?;

    let resolver = Resolver::new();
    let locals = resolver.resolve(&stmts.iter().collect())?;

    interpreter.resolve(locals);

    interpreter.interpret(stmts.iter().collect())?;

	if let Some(LiteralValue::Callable(CallableImpl::PyxisFunction(main_fn))) =
		interpreter.specials.get("__entry__").cloned()
	{
		crate::expr::runPyxisFunction(main_fn, &vec![], interpreter.environment.clone())?;
	}

    return Ok(());
}

#[allow(non_snake_case)]
fn runPrompt() -> Result<(), String> {
    let mut interpreter = Interpreter::new(isProductionMode());
    loop {
        print!("> ");
        match io::stdout().flush() {
            Ok(_) => (),
            Err(_) => return Err("Could not flush stdout".to_string()),
        }

        let mut buffer = String::new();
        let stdin = io::stdin();
        let mut handle = stdin.lock();
        match handle.read_line(&mut buffer) {
            Ok(n) => {
                if n == 0 {
                    println!("");
                    return Ok(());
                } else if n == 1 {
                    continue;
                }
            }
            Err(_) => return Err("Couldnt read line".to_string()),
        }

        println!("ECHO: {}", buffer);
        match run(&mut interpreter, &buffer) {
            Ok(_) => (),
            Err(msg) => println!("{}", msg),
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() == 2 {
        match runFileOnVM(&args[1]) {
            Ok(_) => exit(0),
            Err(msg) => {
                println!("ERROR:\n{}", msg);
                exit(1);
            }
        }
	} else if args.len() == 3 && args[2] == "legacy" {
		match runFile(&args[1]) {
			Ok(_) => exit(0),
			Err(msg) => {
				println!("ERROR:\n{}", msg);
				exit(1);
			}
		}
    } else if args.len() == 3 && args[1] == "e" {
        match runString(&args[2]) {
            Ok(_) => exit(0),
            Err(msg) => {
                println!("ERROR:\n{msg}");
                exit(1);
            }
        }
	} else if args.len() == 4 && args[1] == "e" && args[3] == "legacy" {
		match runString(&args[2]) {
			Ok(_) => exit(0),
			Err(msg) => {
				println!("ERROR:\n{msg}");
				exit(1);
			}
		}
    } else if args.len() == 1 {
        match runPrompt() {
            Ok(_) => exit(0),
            Err(msg) => {
                println!("ERROR\n{}", msg);
                exit(1);
            }
        }
    } else {
        println!("Usage: pyxis [script] [legacy] | pyxis e \"<code>\" [legacy]");
        exit(64);
    }
}
