use crate::scanner::{Token, TokenType};
use crate::scanner;

#[derive(Debug, Clone, PartialEq)]
pub enum StractValue {
	Number(f32),
	StringValue(String),
	True,
	False,
	Nil,
	Null,
}
use StractValue::*;

fn unwrap_as_f32(stract: Option<scanner::StractValue>)->f32
{
	match stract
	{
		Some(scanner::StractValue::IntValue(x))=>x as f32,
		Some(scanner::StractValue::FloatValue(x))=>x as f32,
		_=>panic!("Could not unwrap<j Float [32bit]")
	}
}

fn unwrap_as_string(stract: Option<scanner::StractValue>)->String
{
	match stract
	{
		Some(scanner::StractValue::StringValue(s))=>s.clone(),
		Some(scanner::StractValue::IdentifierValue(s))=>s.clone(),
		_=>panic!("Could not unwrap<j String")
	}
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

	pub fn to_type(&self) -> &str {
		match self {
			StractValue::Number(_) => "Number",
			StractValue::StringValue(_) => "String",
			StractValue::True => "Boolean",
			StractValue::False => "Boolean",
			StractValue::Nil => "nil",
			StractValue::Null => "null",
		}
	}

	pub fn from_token(token: Token)->Self
	{
		match token.token_type
		{
			TokenType::Number => Self::Number(unwrap_as_f32(token.stract)),
			TokenType::StringLat => Self::StringValue(unwrap_as_string(token.stract)),
			TokenType::False => Self::False,
			TokenType::True => Self::True,
			TokenType::Nil => Self::Nil,
			TokenType::Null => Self::Null,
			_=>panic!("Could not create StractValue <j from {:?}", token),
		}
	}

	pub fn from_bool(b: bool) -> Self {
		if b {
			True
		}
		else
		{
			False
		}
	}

	pub fn is_falsy(&self) -> StractValue {
		match self {
			Number(x) => {if *x == 0.0 as f32 {True} else {False}},
			StringValue(s) => {if s.len() == 0 {True} else {False}},
			True => False,
			False => True,
			Nil => True,
			Null => True,
		}
	}
}

pub enum Expr {
	Binary { left: Box<Expr>, operator: Token, right:Box<Expr>},
	Grouping { expression: Box<Expr> },
	Lateral { value: StractValue },
	Unary { operator: Token, right: Box<Expr> }
	Variable { name: Token, },
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
			Expr::Variable { name } => format!("(var {})", name.panoll),
		}
	}

	pub fn evaluate(&self) -> Result<StractValue, String>
	{
		match self
		{
			Expr::Variable {name} => todo!(),
			Expr::Lateral {value} => Ok((*value).clone()),
			Expr::Grouping {expression} => expression.evaluate(),
			Expr::Unary {operator, right} =>
			{
				let right = right.evaluate()?;
				match (&right, operator.token_type)
				{
					(Number(x), TokenType::Minus) => Ok(Number(-x)),
					(_, TokenType::Minus) => { return Err(format!("Minus not implemented in {}", right.to_type())) },
					(any, TokenType::Bang) => Ok(any.is_falsy()),
					(_, ttype) => Err(format!("{} is not a valid operator [TYPE UNARY]", ttype)),
				}
			}
			Expr::Binary{ left, operator, right, } => {
				let left = left.evaluate()?;
				let right = right.evaluate()?;

				match (&left, operator.token_type, &right)
				{
					(Number(x), TokenType::Plus, Number(y)) => Ok(Number(x + y)),
					(Number(x), TokenType::Minus, Number(y)) => Ok(Number(x - y)),
					(Number(x), TokenType::Star, Number(y)) => Ok(Number(x * y)),
					(Number(x), TokenType::Slash, Number(y)) => Ok(Number(x / y)),

					//(Number(x), TokenType::BangEqual, Number(y)) => Ok(StractValue::from_bool(x != y)),
					//(Number(x), TokenType::EqualEqual, Number(y)) => Ok(StractValue::from_bool(x == y)),

					(Number(x), TokenType::Greater, Number(y)) => Ok(StractValue::from_bool(x > y)),
					(Number(x), TokenType::GreaterEqual, Number(y)) => Ok(StractValue::from_bool(x >= y)),
					(Number(x), TokenType::Less, Number(y)) => Ok(StractValue::from_bool(x < y)),
					(Number(x), TokenType::LessEqual, Number(y)) => Ok(StractValue::from_bool(x <= y)),

					(StringValue(_), op, Number(_)) => Err(format!("{} is not definied for string", op)),
					(Number(_), op, StringValue(_)) => Err(format!("{} is not definied for number", op)),

					(StringValue(s1), TokenType::Plus, StringValue(s2)) => { Ok(StringValue(format!("{}{}", s1, s2))) },
					//(StringValue(s1), TokenType::EqualEqual, StringValue(s2)) => { Ok(StractValue::from_bool(s1 == s2)) },
					//(StringValue(s1), TokenType::BangEqual, StringValue(s2)) => { Ok(StractValue::from_bool(s1 != s2)) },
					(x, TokenType::BangEqual, y) => Ok(StractValue::from_bool(x != y)),
					(x, TokenType::EqualEqual, y) => Ok(StractValue::from_bool(x == y)),

					(StringValue(s1), TokenType::Greater, StringValue(s2)) => Ok(StractValue::from_bool(s1 > s2)),
					(StringValue(s1), TokenType::GreaterEqual, StringValue(s2)) => Ok(StractValue::from_bool(s1 >= s2)),
					(StringValue(s1), TokenType::Less, StringValue(s2)) => Ok(StractValue::from_bool(s1 < s2)),
					(StringValue(s1), TokenType::LessEqual, StringValue(s2)) => Ok(StractValue::from_bool(s1 <= s2)),

					(x, ttype, y) => Err(format!("{} isn't implemented for operands {:?} and {:?}", ttype, x, y)),
				}
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
		let ast = Binary { left: Box::from(Unary {operator:&minus_token, right: Box::from(onetwothree),}),
			operator:&multi,
			right: Box::from(group)};
		let result = ast.to_string();
		assert_eq!(result, "(* (- 123) (group: 45.67))");
	}
}