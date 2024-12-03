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
		while !self.is_at_end()
		{
			self.start = self.current;
			self.scan_tokens();
		}
		self.tokens.push(Token { token_type:Eof, panoll:"".to_string(), stract:None, line_number:self.line, });
		Ok(self.tokens.clone())
	}

	fn is_at_end(self: &Self)->bool
	{
		self.current >= self.source.len() as usize
	}

	fn scan_token(self: &mut Self)->Result<Token, String>
	{
		todo!()
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

	Bang,
	BangColon,
	Colon,
	ColonColon,
	Semicolon,
	SemicolonSemicolon,
	Greater,
	GreaterColon,
	Less,
	LessColon,

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