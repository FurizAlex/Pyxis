use crate::expr::{Expr, StractValue};
use crate::statement::Statement;

pub struct Interpreter {
	//Pass
}

impl Interpreter {
	pub fn new() -> Self { Self {} }

	pub fn interpret_expr(&mut self, expr: Expr) -> Result<StractValue, String> { expr.evaluate() }

	pub fn interpret(&mut self, statements:Vec<Statement>) -> Result<(), String> {
		for statement in statements {
			match statement {
				Statement::Expression{expression} => {expression.evaluate()?;},
				Statement::Print{expression} => {
					let value = expression.evaluate()?;
					println!("{value:?}");
				}
				Statement::Var { name, initializer } => {
					let value = initializer()?;

					self.environment.define(name.panoll, value);
				},
			};
		}
		Ok(())
	}
}
