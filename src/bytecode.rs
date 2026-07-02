use crate::expr::LiteralValue;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub enum ConstantValue {
	Number(f64),
	Str(String),
	Bool(bool),
	Nil,
}

#[derive(Debug, Clone, Copy)]
pub enum OpCode {
	Const(usize),
	Add,
	Sub,
	Mul,
	Div,
	Mod,
	Negate,
	Not,
	GetGlobal(usize),
	SetGlobal(usize),
	DefineGlobal(usize),
	Print,
	Pop,
	JumpIfFalse(usize),
	Jump(usize),
	Equal,
	NotEqual,
	Greater,
	GreaterEqual,
	Less,
	LessEqual,
	GetLocal(usize),
	SetLocal(usize),
	GetUpvalue(usize),
	SetUpvalue(usize),
	Call(usize),
	Return,
	Closure(usize),
	CloseUpvalue(usize),
}

#[derive(Debug, Clone, Copy)]
pub enum UpvalueSource {
	Local(usize),
	Upvalue(usize),
}

#[derive(Clone)]
pub struct Closure {
	pub function: Rc<VMFunction>,
	pub upvalues: Vec<Rc<RefCell<crate::expr::LiteralValue>>>,
}

pub struct VMFunction {
	pub name: String,
	pub arity: usize,
	pub chunk: Chunk,
	pub upvalue_sources: Vec<UpvalueSource>,
}

pub struct Chunk {
	pub code: Vec<OpCode>,
	pub constants: Vec<LiteralValue>,
	pub child_functions: Vec<Rc<VMFunction>>,
}

impl Chunk {
	pub fn new() -> Self {
		Self {
			code: vec![],
			constants: vec![],
			child_functions: vec![],
		}
	}

	pub fn emit(&mut self, op: OpCode) {
		self.code.push(op);
	}

	pub fn add_constant(&mut self, value: LiteralValue) -> usize {
		self.constants.push(value);
		self.constants.len() - 1
	}
	
	pub fn add_child_function(&mut self, f: Rc<VMFunction>) -> usize {
		self.child_functions.push(f);
		self.child_functions.len() - 1
	}

	pub fn next_index(&self) -> usize {
		self.code.len()
	}

	pub fn emit_jump_placeholder(&mut self, make_op: impl Fn(usize) -> OpCode) -> usize {
		let index = self.code.len();
		self.code.push(make_op(0));
		index
	}

	pub fn patch_jump(&mut self, placeholder_index: usize, target: usize) {
		match &mut self.code[placeholder_index] {
			OpCode::JumpIfFalse(t) => *t = target,
			OpCode::Jump(t) => *t = target, 
			other => panic!("patch_jump called on a non-jump instruction: {:?}", other),
		}
	}
}