use crate::bytecode::{Chunk, OpCode, UpvalueSource, VMFunction, ConstantValue};
use crate::expr::Expr;
use crate::expr::LiteralValue;
use crate::scanner::TokenType;
use crate::stmt::Stmt;
use std::collections::HashMap;
use std::rc::Rc;

struct FunctionScope {
	chunk: Chunk,
	function_name: String,
	arity: usize,
	locals: Vec<LocalVar>,
	scope_depth: usize,
	upvalues: Vec<UpvalueSource>,
	upvalue_names: HashMap<String, usize>,
}

struct LocalVar {
	name: String,
	depth: usize,
	is_captured: bool,
}

struct LoopContext {
	continue_target: usize,
	break_jumps: Vec<usize>,
}

enum VarLocation {
	Local(usize),
	Upvalue(usize),
	Global,
}

pub struct Compiler {
	chunk: Chunk,
	scopes: Vec<FunctionScope>,
	global_slots: HashMap<String, usize>,
	next_global_slot: usize,
	loop_stack: Vec<LoopContext>,
}

impl Compiler {
	pub fn new() -> Self {
		let top_level = FunctionScope {
			chunk: Chunk::new(),
			function_name: "<script>".to_string(),
			arity: 0,
			locals: vec![],
			scope_depth: 0,
			upvalues: vec![],
			upvalue_names: HashMap::new(),
		};
		Self {
			chunk: Chunk::new(),
			scopes: vec![top_level],
			global_slots: HashMap::new(),
			next_global_slot: 0,
			loop_stack: vec![],
		}
	}

	fn current(&mut self) -> &mut FunctionScope {
		self.scopes.last_mut().expect("Scopes should never be empty")
	}
	
	fn begin_scope(&mut self) {
		self.current().scope_depth += 1;
	}

	fn end_scope(&mut self) {
		let depth = self.current().scope_depth;
		self.current().scope_depth -= 1;

		loop {
			let should_close = {
				let scope = self.current();
				match scope.locals.last() {
					Some(local) if local.depth == depth => {
						Some(local.is_captured)
					}
					_ => None,
				}
			};
			match should_close {
				Some(true) => {
					let slot = self.current().locals.len() - 1;
					self.current().chunk.emit(OpCode::CloseUpvalue(slot));
					self.current().locals.pop();
				}
				Some(false) => {
					self.current().chunk.emit(OpCode::Pop);
					self.current().locals.pop();
				}
				None => break,
			}
		}
	}

	fn declare_local(&mut self, name: &str) -> usize {
		let depth = self.current().scope_depth;
		let slot = self.current().locals.len();
		self.current().locals.push(LocalVar {
			name: name.to_string(),
			depth,
			is_captured: false,
		});
		slot
	}

	fn resolve_local_in_current(&self, name: &str) -> Option<usize> {
		let scope = self.scopes.last().unwrap();

		for (i, local) in scope.locals.iter().enumerate().rev() {
			if local.name == name {
				return Some(i);
			}
		}
		None
	}

	fn resolve_variable(&mut self, name: &str) -> VarLocation {
		if let Some(slot) = self.resolve_local_in_current(name) {
			return VarLocation::Local(slot);
		}
		if let Some(upvalue_index) = self.resolve_upvalue(self.scopes.len() - 1, name) {
			return VarLocation::Upvalue(upvalue_index);
		}
		VarLocation::Global
	}

	pub fn compile(mut self, stmts: &[Stmt]) -> Result<(Chunk, usize), String> {
		for stmt in stmts {
			self.compile_stmt(stmt)?;
		}
		let nil_index = self.scopes[0].chunk.add_constant(LiteralValue::Nil);
		self.scopes[0].chunk.emit(OpCode::Const(nil_index));
		self.scopes[0].chunk.emit(OpCode::Return);
		Ok((self.scopes.remove(0).chunk, self.next_global_slot))
	}

	fn resolve_or_create_global(&mut self, name: &str) -> usize {
		if let Some(&slot) = self.global_slots.get(name) {
			slot
		} else {
			let slot = self.next_global_slot;
			self.global_slots.insert(name.to_string(), slot);
			self.next_global_slot += 1;
			slot
		}
	}
	
	fn resolve_upvalue(&mut self, scope_index: usize, name: &str) -> Option<usize> {
		if scope_index == 0 {
			return None;
		}
		let enclosing_index = scope_index - 1;
		if let Some(&existing) = self.scopes[scope_index].upvalue_names.get(name) {
			return Some(existing);
		}
		let local_slot = {
			let enclosing = &self.scopes[enclosing_index];
			enclosing.locals.iter().enumerate().rev().find(|(_, l)| l.name == name).map(|(i, _)| i)
		};
		if let Some(slot) = local_slot {
			let index = self.add_upvalue(scope_index, name, UpvalueSource::Local(slot));
			return Some(index);
		}
		if let Some(outer_upvalue_index) = self.resolve_upvalue(enclosing_index, name) {
			let index = self.add_upvalue(scope_index, name, UpvalueSource::Upvalue(outer_upvalue_index));
			return Some(index);
		}
		None
	}

	fn add_upvalue(&mut self, scope_index: usize, name: &str, source: UpvalueSource) -> usize {
		if let UpvalueSource::Local(slot) = source {
			let enclosing_index = scope_index - 1;
			self.scopes[enclosing_index].locals[slot].is_captured = true;
		}
		let scope = &mut self.scopes[scope_index];
		let index = scope.upvalues.len();
		scope.upvalues.push(source);
		scope.upvalue_names.insert(name.to_string(), index);
		index
	}

	fn emit_get_variable(&mut self, name: &str) {
		match self.resolve_variable(name) {
			VarLocation::Local(slot) => {
				self.current().chunk.emit(OpCode::GetLocal(slot));
			}
			VarLocation::Upvalue(index) => {
				self.current().chunk.emit(OpCode::GetUpvalue(index));
			}
			VarLocation::Global => {
				let slot = self.resolve_or_create_global(name);
				self.current().chunk.emit(OpCode::GetGlobal(slot));
			}
		}
	}

	fn emit_set_variable(&mut self, name: &str) {
		match self.resolve_variable(name) {
			VarLocation::Local(slot) => {
				self.current().chunk.emit(OpCode::SetLocal(slot));
			}
			VarLocation::Upvalue(index) => {
				self.current().chunk.emit(OpCode::GetUpvalue(index));
			}
			VarLocation::Global => {
				let slot = self.resolve_or_create_global(name);
				self.current().chunk.emit(OpCode::GetGlobal(slot));
			}
		}
	}

	fn literal_to_constant(v: &crate::expr::LiteralValue) -> Result<ConstantValue, String> {
		match v {
			crate::expr::LiteralValue::Number(n) => Ok(ConstantValue::Number(*n)),
			crate::expr::LiteralValue::StringValue(s) => Ok(ConstantValue::Str(s.clone())),
			crate::expr::LiteralValue::True => Ok(ConstantValue::Bool(true)),
			crate::expr::LiteralValue::False => Ok(ConstantValue::Bool(false)),
			crate::expr::LiteralValue::Nil => Ok(ConstantValue::Nil),
			other => Err(format!("Cannot use {} as a compiled constant", other.to_type())),
		}
	}

	fn constant_to_literal(cv: ConstantValue) -> LiteralValue {
		match cv {
			ConstantValue::Number(n) => LiteralValue::Number(n),
			ConstantValue::Str(s) => LiteralValue::StringValue(s),
			ConstantValue::Bool(true) => LiteralValue::True,
			ConstantValue::Bool(false) => LiteralValue::False,
			ConstantValue::Nil => LiteralValue::Nil,
		}
	}

	fn compile_stmt(&mut self, stmt: &Stmt) -> Result<(), String> {
		match stmt {
			Stmt::Var { name, initializer } => {
				self.compile_expr(initializer)?;
				if self.current().scope_depth > 0 {
					self.declare_local(&name.lexeme);
				} else {
					let slot = self.resolve_or_create_global(&name.lexeme);
					self.current().chunk.emit(OpCode::DefineGlobal(slot));
				}
			}
			Stmt::Print { expression } => {
				self.compile_expr(expression)?;
				self.chunk.emit(OpCode::Print);
			}
			Stmt::Expression { expression } => {
				self.compile_expr(expression)?;
				self.chunk.emit(OpCode::Pop);
			}
			Stmt::Block { statements } => {
				self.begin_scope();
				for s in statements {
					self.compile_stmt(s.as_ref())?;
				}
				self.end_scope();
			}
			Stmt::IfStmt { predicate, then, els } => {
				self.compile_expr(predicate)?;
				let jump_to_else = self.current().chunk.emit_jump_placeholder(OpCode::JumpIfFalse);
				self.compile_stmt(then.as_ref())?;

				let jump_past_else = self.current().chunk.emit_jump_placeholder(OpCode::Jump);
				let else_target = self.current().chunk.next_index();
				self.current().chunk.patch_jump(jump_to_else, else_target);
				if let Some(els_stmt) = els {
					self.compile_stmt(els_stmt.as_ref())?;
				}
				let after_target = self.chunk.next_index();
				self.chunk.patch_jump(jump_past_else, after_target);
			}
			Stmt::WhileStmt { condition, body } => {
				let loop_start = self.chunk.next_index();
				self.compile_expr(condition)?;
				let jump_to_end = self.chunk.emit_jump_placeholder(OpCode::JumpIfFalse);
				self.loop_stack.push(LoopContext { continue_target: loop_start, break_jumps: vec![], });
				self.compile_stmt(body.as_ref())?;
				self.current().chunk.emit(OpCode::Jump(loop_start));
				let end_target = self.chunk.next_index();
				self.chunk.patch_jump(jump_to_end, end_target);
				let ctx = self.loop_stack.pop().expect("loop stack should not be empty");
				for break_index in ctx.break_jumps {
					self.current().chunk.patch_jump(break_index, end_target);
				}
			}
			Stmt::ForStmt { variable, iterable, body } => {
				return Err("for-loops require function calls (range() which aren't implemented in VM yet)".to_string());
			}
			Stmt::BreakStmt { keyword: _ } => {
				if self.loop_stack.is_empty() {
					return Err("'break is not allowed outside of a loop".to_string());
				}
				let jump_index = self.current().chunk.emit_jump_placeholder(OpCode::Jump);
				self.loop_stack.last_mut().unwrap().break_jumps.push(jump_index);
			}
			Stmt::ContinueStmt { keyword: _ } => {
				if self.loop_stack.is_empty() {
					return Err("'continue' is not allowed outside of a loop".to_string());
				}
				let target = self.loop_stack.last().unwrap().continue_target;
				self.current().chunk.emit(OpCode::Jump(target));
			}
			Stmt::ReturnStmt { keyword: _, value } => {
				if let Some(v) = value {
					self.compile_expr(v)?;
				} else {
					let index = self.current().chunk.add_constant(LiteralValue::Nil);
					self.current().chunk.emit(OpCode::Const(index));
				}
				self.current().chunk.emit(OpCode::Return);
			}
			Stmt::Function { name, params, body } => {
				self.compile_function(&name.lexeme, &params.iter().map(|t| t.lexeme.clone()).collect::<Vec<_>>(), &body.iter().map(|b| b.as_ref()).collect::<Vec<_>>(),)?;
				if self.current().scope_depth > 0 {
					self.declare_local(&name.lexeme);
				} else {
					let slot = self.resolve_or_create_global(&name.lexeme);
					self.current().chunk.emit(OpCode::DefineGlobal(slot));
				}
			}
			Stmt::Block { statements } => {
				for s in statements {
					self.compile_stmt(s.as_ref())?;
				}
			}
			other => {
				return Err(format!("VM Compiler: Unsupported statement {}", other.to_string()));
			}
		}
		Ok(())
	}
	
	fn compile_expr(&mut self, expr: &Expr) -> Result<(), String> {
		match expr {
			Expr::Literal { id: _, value } => {
				let index = self.current().chunk.add_constant(value.clone());
				self.current().chunk.emit(OpCode::Const(index));
			}
			Expr::Variable { id: _, name } => {
				self.emit_get_variable(&name.lexeme);
			}
			Expr::Assign { id: _, name, value } => {
				self.compile_expr(value)?;
				self.emit_set_variable(&name.lexeme);
			}
			Expr::Grouping { id: _, expression } => {
				self.compile_expr(expression)?;
			}
			Expr::Unary { id: _, operator, right } => {
				self.compile_expr(right)?;
				match operator.token_type {
					TokenType::Minus => self.current().chunk.emit(OpCode::Negate),
					TokenType::Bang => self.current().chunk.emit(OpCode::Not),
					other => return Err(format!("Unsupported unary operator: {:?}", other)),
				}
			}
			Expr::Binary { id: _, left, operator, right } => {
				self.compile_expr(left)?;
				self.compile_expr(right)?;
				match operator.token_type {
					TokenType::Plus => self.chunk.emit(OpCode::Add),
					TokenType::Minus => self.chunk.emit(OpCode::Sub),
					TokenType::Star => self.chunk.emit(OpCode::Mul),
					TokenType::Slash => self.chunk.emit(OpCode::Div),
					TokenType::Percent => self.chunk.emit(OpCode::Mod),
					TokenType::EqualEqual => self.chunk.emit(OpCode::Equal),
					TokenType::BangEqual => self.chunk.emit(OpCode::NotEqual),
					TokenType::Greater => self.chunk.emit(OpCode::Greater),
					TokenType::GreaterEqual => self.chunk.emit(OpCode::GreaterEqual),
					TokenType::Less => self.chunk.emit(OpCode::Less),
					TokenType::LessEqual => self.chunk.emit(OpCode::LessEqual),
					other => {
						return Err(format!("Unsupported binary operator: {:?}", other))
					}
				}
			}
			Expr::Call { id: _, callee, paren: _, arguments } => {
				self.compile_expr(callee)?;
				let argument_count = arguments.len();
				for arg in arguments {
					self.compile_expr(arg)?;
				}
				self.current().chunk.emit(OpCode::Call(argument_count));
			}
			Expr::Logical { id: _, left, operator, right } => {
				match operator.token_type {
					TokenType::And => {
						self.compile_expr(left)?;
						let short_circuit = self.current().chunk.emit_jump_placeholder(OpCode::JumpIfFalse);
						self.current().chunk.emit(OpCode::Pop);
						self.compile_expr(right)?;
						let after = self.current().chunk.next_index();
						self.current().chunk.patch_jump(short_circuit, after);
					}
					TokenType::Or => {
						self.compile_expr(left)?;
						let check_false = self.current().chunk.emit_jump_placeholder(OpCode::JumpIfFalse);
						let skip_right = self.current().chunk.emit_jump_placeholder(OpCode::Jump);
						let right_start = self.current().chunk.next_index();

						self.current().chunk.emit(OpCode::Pop);
						self.compile_expr(right)?;
						let after = self.current().chunk.next_index();
						self.current().chunk.patch_jump(skip_right, after);
					}
					_ => return Err("Invalid logical operator".to_string()),
				}
			}
			other => {
				return Err(format!("VM Compiler: Unsupported expression: {}", other.to_string()))
			}
		}
		Ok(())
	}

	fn compile_function(&mut self, name: &str, params: &[String], body: &[&Stmt],) -> Result<(), String> {
		self.scopes.push(FunctionScope {
			chunk: Chunk::new(),
			function_name: name.to_string(),
			arity: params.len(),
			locals: vec![],
			scope_depth: 1,

			upvalues: vec![],
			upvalue_names: HashMap::new(),
		});

		for param in params {
			self.declare_local(param);
		}
		for stmt in body {
			self.compile_stmt(stmt)?;
		}
		let nil_index = self.current().chunk.add_constant(LiteralValue::Nil);
		self.current().chunk.emit(OpCode::Const(nil_index));
		self.current().chunk.emit(OpCode::Return);

		let fn_scope = self.scopes.pop().unwrap();
		let upvalue_sources = fn_scope.upvalues.clone();
		let compiled_fn = Rc::new(VMFunction {
			name: name.to_string(),
			arity: fn_scope.arity,
			chunk: fn_scope.chunk,
			upvalue_sources,
		});
		let fn_index = self.current().chunk.add_child_function(compiled_fn);
		self.current().chunk.emit(OpCode::Closure(fn_index));
		Ok(())
	}
}