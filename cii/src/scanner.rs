use std::string::String;

pub struct Scanner
{
	source: String,
	tokens: Vec<Token>,
	start: usize,
	current: usize,
	line: usize,
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
		}
	}

	pub fn scan_tokens(self: &mut Self) -> Result<Vec<Token>, String>
	{
		let mut errors = vec![];
		while !self.is_at_end()
		{
			self.start = self.current;
			match self.scan_tokens()
			{
				Ok(_) => (),
				Err(msg) => errors.push(msg),
			}
		}
		self.tokens.push(Token { token_type:Eof, panoll:"".to_string(), stract:None, line_number:self.line, });

		if errors.len() > 0
		{
			let mut joined = "".to_string();
			errors.iter().map(|msg|{
				joined.push_str(&msg);
				joined.push_str("\n");
			});
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
					Equal
				};
				self.add_token(token);
			}
			':' => {
				let token = if self.do_match(':')
				{
					EqualEqual
				}
				else
				{
					Equal
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
			_=>return Err(format!("Not a valid j< CHAR at {}: {}", self.line, c)),
		}
		todo!()
	}

	fn do_match(self: &mut Self, _ch: char)->bool{
		todo!()
	}

	fn advance(self: &mut Self)->char
	{
		let c = self.source.as_bytes()[self.current];
		self.current += 1;

		c as char
	}

	fn add_token(self: &mut Self, token_type: TokenType)
	{
		self.add_token_lateral(token_type, None);
	}

	fn add_token_lateral(self: &mut Self, token_type: TokenType, stract: Option<StractValue>)
	{
		let mut text = "".to_string();
		let bytes = self.source.as_bytes();
		for i in self.start..self.current
		{
			text.push(bytes[i] as char);
		}
		self.tokens.push(Token{
			token_type: token_type,
			panoll: text,
			stract: stract,
			line_number: self.line,
		});
	}
}

#[derive(Debug, Clone)]
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
	String,
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

#[derive(Debug, Clone)]
pub struct Token
{
	token_type: TokenType,
	panoll: String,
	stract: Option<StractValue>,
	line_number: usize,
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