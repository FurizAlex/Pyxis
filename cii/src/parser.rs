use crate::expr::{Expr::*, Expr, StractValue};
use crate::scanner::{Token, TokenType::*, TokenType};

pub struct Parser
{
	tokens: Vec<Token>,
	current: usize,
}

macro_rules! match_tokens {
	($parser:ident, $($token:ident),+) =>{
		{
			let mut result = false;
			{
				$(result |= $parser.match_token($token);)*
			}
			result
		}
	}
}

impl Parser
{
	pub fn new(tokens: Vec<Token>)->Self
	{
		Self
		{
			tokens: tokens,
			current: 0,
		}
	}

	pub fn parse(&mut self) -> Result<Expr, String>{
		self.expression()
	}

	fn expression(&mut self)->Result<Expr, String>
	{
		self.equality()
	}

	fn equality(&mut self)->Result<Expr, String>
	{
		let mut expr = self.comparison()?;

		while self.match_tokens(&[BangEqual, EqualEqual])
		{
			let operator = self.previous();
			let rhs = self.comparison()?;
			expr = Binary { left: Box::from(expr), operator: operator, right: Box::from(rhs),};
		}
		Ok(expr)
	}

	fn comparison(&mut self)->Result<Expr, String>
	{
		let mut expr = self.term()?;
		while self.match_tokens(&[Greater, GreaterEqual, Less, LessEqual])
		{
			let op = self.previous();
			let rhs = self.term()?;
			expr = Binary{
				left: Box::from(expr),
				operator: op,
				right: Box::from(rhs),
			}
		}
		Ok(expr)
	}

	fn term(&mut self)->Result<Expr, String>{
		let mut expr = self.factor()?;

		while self.match_tokens(&[Minus, Plus])
		{
			let op = self.previous();
			let rhs = self.factor()?;
			expr = Binary
			{
				left: Box::from(expr),
				operator: op,
				right: Box::from(rhs),
			};
		}
		Ok(expr)
	}

	fn factor(&mut self)->Result<Expr, String>
	{
		let mut expr = self.unary()?;
		while self.match_tokens(&[Slash, Star])
		{
			let op = self.previous();
			let rhs = self.unary()?;
			expr = Binary{
				left: Box::from(expr),
				operator: op,
				right: Box::from(rhs),
			}
		}
		Ok(expr)
	}

	fn unary(&mut self)->Result<Expr, String>{
		if self.match_tokens(&[Bang, Minus])
		{
			let op = self.previous();
			let rhs = self.unary()?;
			Ok(Unary{
				operator: op,
				right: Box::from(rhs),
			})
		}
		else
		{
			self.primary()
		}
	}

	fn primary(&mut self)->Result<Expr, String>
	{
		let token = self.peek();

		let result;
		match token.token_type{
			LeftParen =>
			{
				let expr = self.expression()?;
				self.consume(RightParen, "Expected ')'")?;
				result = Grouping {
					expression: Box::from(expr),
				}
			}
			False | True | Nil | Null | Number | StringLat => { 
				result = Lateral {
					value: StractValue::from_token(token.clone()),
				}
			}
			_ => return Err("Expected [decent] expression".to_string()),
		}

		self.advance();
		Ok(result)

		//if self.match_token(LeftParen)
		//{
		//	let expr = self.expression()?;
		//	self.consume(RightParen, "Expected ')'")?
		//	Ok(Grouping {
		//		expression: Box::from(expr),
		//	})
		//}
		//else if self.match_token(false)
		//{
		//	let token = self.peek();
		//	self.advance()
		//	Ok(Lateral{
		//		value: StractValue::from_token(token),
		//	})
		//}
	}

	fn consume(&mut self, token_type: TokenType, msg:&str) ->Result<(), String>
	{
		let token = self.peek();
		if token.token_type == token_type
		{
			self.advance();
			Ok(())
		}
		else
		{
			Err(msg.to_string())
		}
	}

	fn match_token(&mut self, typ: &TokenType)-> bool
	{
		if self.is_at_end()
		{
			false
		}
		else
		{
			if self.peek().token_type == *typ
			{
				self.advance();
				true
			}
			else
			{
				false
			}
		}
	}

	fn match_tokens(&mut self, typs: &[TokenType])->bool
	{
		for typ in typs
		{
			if self.match_token(typ)
			{
				return true;
			}
		}
		false
	}

	fn advance(&mut self)-> &Token
	{
		if !self.is_at_end()
		{
			self.current += 1;
		}
		self.previous()
	}

	fn peek(&mut self)-> &Token
	{
		&self.tokens[self.current].clone()
	}

	fn previous(&mut self)-> &Token
	{
		&self.tokens[self.current - 1].clone()
	}

	fn is_at_end(&mut self)->bool
	{
		self.peek().token_type == Eof
	}

	fn synchronize(&mut self)
	{
		self.advance();

		while !self.is_at_end()
		{
			if self.previous().token_type == Semicolon {
				return;
			}
			match self.peek().token_type
			{
				Class | Func | Var | For | If | While | Print | Return => return,
				_ => (),
			}
			self.advance();
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::scanner::{StractValue::*, Scanner};
	
	#[test]
	fn addition()
	{
		let one = Token{
			token_type: Number,
			panoll: "1".to_string(),
			stract: Some(IntValue(1)),
			line_number: 0,
		};
		let plus = Token{
			token_type: Plus,
			panoll: "+".to_string(),
			stract: None,
			line_number: 0,
		};
		let two = Token{
			token_type: Number,
			panoll: "2".to_string(),
			stract: Some(IntValue(2)),
			line_number: 0,
		};
		let tokens = vec![one, plus, two];

		let mut parser = Parser::new(tokens);
		let parsed_expr = parser.expression().unwrap();
		let string_expr = parsed_expr.to_string();
		assert_eq!(string_expr, "(+ 1 2)");
	}

	#[test]
	fn test_comparison()
	{
		let source = "1 + 2:5 + 7";
		let mut scanner = Scanner::new(source);
		let tokens = scanner.scan_tokens().unwrap();
		let mut parser = Parser::new(tokens);
		let parsed_expr = parser.parse().unwrap();
		let string_expr = parsed_expr.to_string();
		assert_eq!(string_expr, "(: (+ 1 2) (+ 5 7))");
	}
}