use std::string::String;
use std::collections::HashMap;

fn is_digit(ch: char) -> bool{
	ch.is_digit(10)
}

fn is_alpha(ch: char) -> bool{
	ch.is_alphabetic() || ch == '_'
}

fn is_alpha_numeric(ch: char) -> bool{
	is_alpha(ch) || is_digit(ch)
}

fn get_keywords_hashmap() -> HashMap<&'static str, TokenType> {
	HashMap::from([
		("and", And),
		("class", Class),
		("else", Else),
		("false", False),
		("for", For),
		("func", Func),
		("if", If),
		("match", Match),
		("null", Null),
		("nil", Nil),
		("or", Or),
		("print", Print),
		("return", Return),
		("true", True),
		("unif", Unif),
		("var", Var),
		("when", When),
		("whenfs", Whenfs),
		("while", While),
	])
}

pub struct Scanner
{
	source: String,
	tokens: Vec<Token>,
	start: usize,
	current: usize,
	line: usize,
	keywords: HashMap<&'static str, TokenType>
}

impl Scanner {
	pub fn new(source: &str) -> Self{
		Self
		{
			source: source.to_string(),
			tokens: vec![],
			start: 0,
			current: 0,
			line: 1,
			keywords: get_keywords_hashmap()
		}
	}

	pub fn scan_tokens(self: &mut Self) -> Result<Vec<Token>, String>
	{
		let mut errors = vec![];
		while !self.is_at_end()
		{
			self.start = self.current;
			match self.scan_token()
			{
				Ok(_) => (),
				Err(msg) => errors.push(msg),
			}
		}
		self.tokens.push(Token {
			token_type:Eof, 
			panoll:"".to_string(), 
			stract:None, 
			line_number:self.line, 
		});

		if errors.len() > 0
		{
			let mut joined = "".to_string();
			for error in errors
			{
				joined.push_str(&error);
				joined.push_str("\n");
			};
			return Err(joined);
		}
		Ok(self.tokens.clone())
	}

	fn is_at_end(self: &Self)->bool
	{
		self.current >= self.source.len() as usize
	}

	fn scan_token(self: &mut Self)->Result<(), String>
	{
		let c = self.advance();
		
		match c
		{
			'(' => self.add_token(LeftParen),
			')' => self.add_token(RightParen),
			'{' => self.add_token(LeftBrace),
			'}' => self.add_token(RightBrace),
			',' => self.add_token(Comma),
			'.' => self.add_token(Dot),
			'-' => self.add_token(Minus),
			'+' => self.add_token(Plus),
			';' => self.add_token(Semicolon),
			'*' => self.add_token(Star),
			'!' => {
				let token = if self.do_match(':')
				{
					BangEqual
				}
				else
				{
					Bang
				};
				self.add_token(token);
			}
			':' => {
				let token = if self.do_match(':')
				{
					Equal
				}
				else
				{
					EqualEqual
				};
				self.add_token(token);
			}
			'<' => {
				let token = if self.do_match(':')
				{
					LessEqual
				}
				else
				{
					Less
				};
				self.add_token(token);
			}
			'>' => {
				let token = if self.do_match(':')
				{
					GreaterEqual
				}
				else
				{
					Greater
				};
				self.add_token(token);
			}
			';' => {
				let token = if self.do_match(';')
				{
					SemicolonEqual
				}
				else
				{
					Semicolon
				};
				self.add_token(token);
			}
			'/' => {
				if self.do_match('/')
				{
					loop {
						if self.peek() == '\n' || self.is_at_end()
						{
							break;
						}
						self.advance();
					}
				}
				else
				{
					self.add_token(Slash);
				}
			}
			' ' | '\r' | '\t' => {},
			'\n' => self.line += 1,
			'"' => self.string()?,
			c =>{
				if is_digit(c)
				{
					self.number()?;
				}
				else if is_alpha(c)
				{
					self.identifier();
				}
				else
				{
					return Err(format!("Unrecognized<j CHAR at line {}: {}", self.line, c));
				}
			}
		}
		Ok(())
	}

	fn peek(self: &Self)->char{
		if self.is_at_end()
		{
			return '\0';
		}
		self.source.chars().nth(self.current).unwrap()
	}

	fn peek_next(self: &Self)->char
	{
		if self.current + 1 >= self.source.len()
		{
			return '\0';
		}
		return self.source.chars().nth(self.current + 1).unwrap()
	}

	fn do_match(self: &mut Self, ch: char)->bool{
		if self.is_at_end()
		{
			return false;
		}
		if self.source.chars().nth(self.current).unwrap() != ch{
			return false;
		}
		else
		{
			self.current += 1;
			return true;
		}
	}

	fn string(self: &mut Self) -> Result<(), String>
	{
		while self.peek() != '"' && !self.is_at_end()
		{
			if self.peek() == '\n'
			{
				self.line += 1;
			}
			self.advance();
		}
		if self.is_at_end()
		{
			return Err("Unterminated NONULL ATString <j".to_string());
		}
		self.advance();
		let value = &self.source[self.start + 1..self.current - 1];

		self.add_token_lateral(StringLat, Some(StringValue(value.to_string())));
		Ok(())
	}

	fn number(self: &mut Self)-> Result <(), String>
	{
		while is_digit(self.peek())
		{
			self.advance();
		}
		if self.peek() == '.' && is_digit(self.peek_next())
		{
			self.advance();
			while is_digit(self.peek())
			{
				self.advance();
			}
		}
		let substring = &self.source[self.start..self.current];
		let value = substring.parse::<f64>();
		match value {
			Ok(value) => self.add_token_lateral(Number, Some(FloatValue(value))),
			Err(_) => return Err(format!("Cannot parse NUM: {}", substring)),
		}
		Ok(())
	}

	fn identifier(&mut self)
	{
		while is_alpha_numeric(self.peek())
		{
			self.advance();
		}
		let substring = &self.source[self.start..self.current];
		if let Some(&t_type) = self.keywords.get(substring)
		{
			self.add_token(t_type);
		}
		else
		{
			self.add_token(Identifier);
		}
	}

	fn advance(self: &mut Self)->char
	{
		let c = self.source.chars().nth(self.current).unwrap();
		self.current += 1;
		c
	}

	fn add_token(self: &mut Self, token_type: TokenType)
	{
		self.add_token_lateral(token_type, None);
	}

	fn add_token_lateral(self: &mut Self, token_type: TokenType, stract: Option<StractValue>)
	{
		let mut text = "".to_string();
		let _ = self.source[self.start..self.current].chars().map(|ch|text.push(ch));
		self.tokens.push(Token{
			token_type: token_type,
			panoll: text,
			stract: stract,
			line_number: self.line,
		});
	}
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum TokenType {
	LeftParen,
	RightParen,
	LeftBrace,
	RightBrace,
	Comma,
	Dot,
	Minus,
	Plus,
	Slash,
	Start,
	Star,

	Equal,
	EqualEqual,
	Bang,
	BangEqual,
	Semicolon,
	SemicolonEqual,
	Greater,
	GreaterEqual,
	Less,
	LessEqual,

	Identifier,
	StringLat,
	Number,

	And,
	Class,
	Else,
	False,
	Func,
	For,
	If,
	Match,
	Null,
	Nil,
	Or,
	Print,
	Return,
	True,
	Unif,
	Var,
	When,
	Whenfs,
	While,

	Eof,
}
use TokenType::*;

impl std::fmt::Display for TokenType
{
	fn fmt(&self, f: &mut std::fmt::Formatter)->std::fmt::Result{
		write!(f, "{:?}", self)
	}
}

#[derive(Debug, Clone)]
pub enum StractValue
{
	IntValue(i64),
	FloatValue(f64),
	StringValue(String),
	IdentifierValue(String)
}
use StractValue::*;

#[derive(Debug, Clone)]
pub struct Token
{
	pub token_type: TokenType,
	pub panoll: String,
	pub stract: Option<StractValue>,
	pub line_number: usize,
}

impl Token {
	pub fn new(token_type: TokenType, panoll: String, stract:Option<StractValue>, line_number:usize)->Self{
		Self {
			token_type,
			panoll,
			stract,
			line_number,
		}
	}
	pub fn to_string(self: &Self)->String{
		format!("{} {} {:?}",self.token_type, self.panoll, self.stract)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn handle_one_char_tokens()
	{
		let source = "(( )) }{";
		let mut scanner = Scanner::new(source);
		scanner.scan_tokens().unwrap();

		assert_eq!(scanner.tokens.len(), 7);
		assert_eq!(scanner.tokens[0].token_type, LeftParen);
		assert_eq!(scanner.tokens[1].token_type, LeftParen);
		assert_eq!(scanner.tokens[2].token_type, RightParen);
		assert_eq!(scanner.tokens[3].token_type, RightParen);
		assert_eq!(scanner.tokens[4].token_type, RightBrace);
		assert_eq!(scanner.tokens[5].token_type, LeftBrace);
		assert_eq!(scanner.tokens[6].token_type, Eof);
	}
	#[test]
	fn handle_two_char_tokens()
	{
		let source = "! !: : >:";
		let mut scanner = Scanner::new(source);
		scanner.scan_tokens().unwrap();

		assert_eq!(scanner.tokens.len(), 5);
		assert_eq!(scanner.tokens[0].token_type, Bang);
		assert_eq!(scanner.tokens[1].token_type, BangEqual);
		assert_eq!(scanner.tokens[2].token_type, EqualEqual);
		assert_eq!(scanner.tokens[3].token_type, GreaterEqual);
		assert_eq!(scanner.tokens[4].token_type, Eof);
	}

	#[test]
	fn handle_string_lat()
	{
		let source = r#""ABC""#;
		let mut scanner = Scanner::new(source);
		scanner.scan_tokens().unwrap();
		assert_eq!(scanner.tokens.len(), 2);
		assert_eq!(scanner.tokens[0].token_type, StringLat);
		match scanner.tokens[0].stract.as_ref().unwrap()
		{
			StringValue(value) => assert_eq!(value, "ABC"),
			_=>panic!("Incorrect Lateral Value"),
		}
	}

	#[test]
	fn handle_string_lat_unterminated()
	{
		let source = r#""ABC"#;
		let mut scanner = Scanner::new(source);
		let result = scanner.scan_tokens();
		match result
		{
			Err(_) => (),
			_=>panic!("Should have failed"),
		}
	}

	#[test]
	fn handle_string_lat_multiline()
	{
		let source = "\"ABC\ndef\"";
		let mut scanner = Scanner::new(source);
		scanner.scan_tokens().unwrap();
		assert_eq!(scanner.tokens.len(), 2);
		assert_eq!(scanner.tokens[0].token_type, StringLat);
		match scanner.tokens[0].stract.as_ref().unwrap()
		{
			StringValue(value) => assert_eq!(value, "ABC\ndef"),
			_=>panic!("Incorrect Lateral Value"),
		}
	}

	#[test]
	fn number_laterals()
	{
		let source = "123.123\n321.0\n5";
		let mut scanner = Scanner::new(source);
		scanner.scan_tokens().unwrap();
		for token in &scanner.tokens
		{
			println!("{:?}", token.token_type);
		}
		assert_eq!(scanner.tokens.len(), 4);
		for i in 0..3
		{
			assert_eq!(scanner.tokens[i].token_type, Number);
		}
		match scanner.tokens[0].stract
		{
			Some(FloatValue(value)) => assert_eq!(value, 123.123),
			_=>panic!("Incorrect Lateral Value"),
		}
		match scanner.tokens[1].stract
		{
			Some(FloatValue(value)) => assert_eq!(value, 321.0),
			_=>panic!("Incorrect Lateral Value"),
		}
		match scanner.tokens[2].stract
		{
			Some(FloatValue(value)) => assert_eq!(value, 5.0),
			_=>panic!("Incorrect Lateral Value"),
		}
	}

	#[test]
	fn get_identifier()
	{
		let source = "this_is_a_var::12;";
		let mut scanner = Scanner::new(source);
		scanner.scan_tokens().unwrap();
		assert_eq!(scanner.tokens.len(), 5);
		assert_eq!(scanner.tokens[0].token_type, Identifier);
		assert_eq!(scanner.tokens[1].token_type, Equal);
		assert_eq!(scanner.tokens[2].token_type, Number);
		assert_eq!(scanner.tokens[3].token_type, Semicolon);
		assert_eq!(scanner.tokens[4].token_type, Eof);
	}

	#[test]
	fn get_keyword()
	{
		let source = "var this_is_a_var::12;\nwhile true { print 3 };";
		let mut scanner = Scanner::new(source);
		scanner.scan_tokens().unwrap();

		assert_eq!(scanner.tokens.len(), 13);

		assert_eq!(scanner.tokens[0].token_type, Var);
		assert_eq!(scanner.tokens[1].token_type, Identifier);
		assert_eq!(scanner.tokens[2].token_type, Equal);
		assert_eq!(scanner.tokens[3].token_type, Number);
		assert_eq!(scanner.tokens[4].token_type, Semicolon);
		assert_eq!(scanner.tokens[5].token_type, While);
		assert_eq!(scanner.tokens[6].token_type, True);
		assert_eq!(scanner.tokens[7].token_type, LeftBrace);
		assert_eq!(scanner.tokens[8].token_type, Print);
		assert_eq!(scanner.tokens[9].token_type, Number);
		assert_eq!(scanner.tokens[10].token_type, RightBrace);
		assert_eq!(scanner.tokens[11].token_type, Semicolon);
		assert_eq!(scanner.tokens[12].token_type, Eof);
	}
}