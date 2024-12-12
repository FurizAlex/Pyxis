use crate::scanner::{Token, TokenType};

pub enum StractValue {
	Number(f32),
	StringValue(String),
	True,
	False,
	Nil,
	Null,
}

impl StractValue {
	pub fn to_string(&self) -> String {
		match self {
			StractValue::Number(x) => x.to_string(),
			StractValue::StringValue(x) => x.clone(),
			StractValue::True => "true".to_string(),
			StractValue::False => "false".to_string(),
			StractValue::Nil => "nil".to_string(),
			StractValue::Null => "null".to_string(),
		}
	}
}

pub enum Expr {
	Binary { left: Box<Expr>, operator: Token, right:Box<Expr>},
	Grouping { expression: Box<Expr> },
	Lateral { value: StractValue },
	Unary { operator: Token, right: Box<Expr> }
}

impl Expr {
	pub fn to_string(&self) -> String {
		match self {
			Expr::Binary { left, operator, right, } => format!("({} {} {})", operator.panoll, left.to_string(), right.to_string()),
			Expr::Grouping { expression } => format!("(group: {})", (*expression).to_string()),
			Expr::Lateral { value } => format!("{}", value.to_string()),
			Expr::Unary { operator, right } => {
				let operator_str = operator.panoll.clone();
				let right_str = (*right).to_string();
				format!("({} {})", operator_str, right_str)
			}
		}
	}

	pub fn print(&self) {
		println!("{}", self.to_string());
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use super::Expr::*;
	use super::StractValue::*;

	#[test]
	fn pretty_print_ast()
	{
		let minus_token = Token {
			token_type: TokenType::Minus, 
			panoll: "-".to_string(), 
			stract: None, 
			line_number: 0,
		};
		let onetwothree = Lateral { 
			value: Number(123.0) };
		let group = Grouping { 
			expression: Box::from(Lateral {value: Number(45.67)})};
		let multi = Token { 
			token_type: TokenType::Star,
			panoll: "*".to_string(), 
			stract: None,
			line_number: 0};
		let ast = Binary { left: Box::from(Unary {operator:minus_token, right: Box::from(onetwothree),}),
			operator:multi,
			right: Box::from(group)};
		let result = ast.to_string();
		assert_eq!(result, "(* (- 123) (group: 45.67))");
	}
}