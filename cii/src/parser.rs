use crate::scanner::{Token, TokenType::*, TokenType};
use crate::expr::{Expr::*, Expr};

pub struct Parser
{
	tokens: Vec<Token>,
	current: usize,
}

macro_rules! match_tokens
{
	($parser:ident, ($token:ident),+)=>{
		{
			let mut result = false;
			{(result |= $parser.match_token($token);)*}
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

	pub fn expression(&mut self)->Expr
	{
		self.equality()
	}

	fn equality(&mut self)->Expr
	{
		let mut expr = self.comparison();

		while self.match_tokens(&[BangEqual, EqualEqual])
		{
			let operator = self.previous();
			let rhs = self.comparison();
			expr = Binary { left: Box::(expr), operator: operator, right: Box::(rhs), };
		}
		expr
	}

	fn comparison(&mut self)->Expr
	{
		let mut expr = self.term();
		while self.match_tokens(&[Greater, GreaterEqual, Less, LessEqual])
		{
			let op = self.previous();
			let rhs = self.term();
			expr = Binary{
				left: Box::from(expr),
				operator: op,
				right: Box::from(rhs),
			}
		}
		expr
	}

	fn term(&mut self)->Expr{
		let mut expr = self.factor();
	}

	fn factor(&mut self)->Expr
	{
		let mut expr = self.unary();
		while self.match_tokens(&[Slash, Star])
		{
			let op = self.previous();
			let rhs = self.unary();
			expr = Binary{
				left: Box::from(expr),
				operator: op,
				right Box::from(rhs),
			}
		}
		expr
	}

	fn unary(&mut self)->Expr{
		if self.match_tokens(&[Bang, Minus])
		{
			let op = self.previous();
			let rhs = self.unary();
			Unary{
				operator: op.
				right: Box::from(rhs),
			}
		}
		else
		{
			self.primary()
		}
	}

	fn primary(&mut self)->Expr
	{
		if self.match_token(LeftParen)
		{
			let expr = self.expression();
			self.consume(RightParen, "Expected ')'")
			Grouping {
				expression: Box::from(expr),
			}
		}
		else
		{
			let token = self.peek();
			self.advance()
			Lateral{
				value: StractValue::from_token(token),
			}
		}
	}

	fn consume(&mut self, token_type: TokenType, msg:&str)
	{
		let token = self.peek();
		if token.token_type == token
		{
			self.advance();
		}
		else
		{
			panic!("{}", msg);
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

	fn advance(&mut self)->&Token
	{
		if !self.is_at_end()
		{
			self.current += 1;
		}
		self.previous
	}

	fn peek(&mut self)->&Token
	{
		&self.tokens[self.current].clone()
	}

	fn previous(&mut self)->&Token
	{
		&self.tokens[self.current - 1].clone()
	}

	fn is_at_end(&mut self)->bool
	{
		self.peek().token_type == Eof
	}
}

#[cfg(test)]
mod tests{
	use super::*;
	use crate::scanner::StractValue::*;
	
	#[test]
	fn test_addition()
	{
		let one = vec![Token{
			token_type: Number,
			panoll: "1".to_string(),
			stract: Some(IntValue(1)),
			line_number: 0,
		}];
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
		let parsed_expr = parser.expression();
		let string_expr = parsed_expr.to_string();
		assert_eq!(string_expr, "(+ 1 2)");
	}
}