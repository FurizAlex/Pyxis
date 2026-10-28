use crate::bytecode::{Chunk, OpCode, UpvalueSource, VMFunction, ConstantValue};
use crate::expr::Expr;
use crate::scanner::TokenType;
use crate::stmt::Stmt;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

struct FunctionScope {
	chunk: Chunk,
	#[allow(dead_code)]
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
	// @wrafs: this local is just another name for `alias`. Its own stack slot is a dummy.
	alias: Option<LinkTarget>,
}

// Identity of a piece of storage, independent of which function is looking at it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum LinkTarget {
	Global(usize),
	Local { scope: usize, slot: usize },
}

struct LoopContext {
	// `while` knows its continue target up front; `for` has to patch it (it must hit ForIterNext)
	continue_target: Option<usize>,
	continue_jumps: Vec<usize>,
	break_jumps: Vec<usize>,
	// number of locals that existed when the body started; anything above is popped on break/continue
	locals_len: usize,
}

#[derive(Clone, Copy)]
enum VarLocation {
	Local(usize),
	Upvalue(usize),
	Global(usize),
}

pub struct Compiler {
	scopes: Vec<FunctionScope>,
	global_slots: HashMap<String, usize>,
	next_global_slot: usize,
	loop_stack: Vec<LoopContext>,
	is_production: bool,
	// name -> is_ghost, one frame per lexical block (frame 0 = globals)
	ghost_frames: Vec<HashMap<String, bool>>,
	// @hotlink: writing the key also writes the value (never the other way round)
	hotlinks: HashMap<LinkTarget, LinkTarget>,
	// global names that currently share a slot because of @wrafs
	global_aliased: HashSet<String>,
}

fn stmt_kind(stmt: &Stmt) -> &'static str {
	match stmt {
		Stmt::Expression { .. } => "expression",
		Stmt::Print { .. } => "print",
		Stmt::Var { .. } => "var",
		Stmt::WrafsVar { .. } => "@wrafs",
		Stmt::HotlinkVar { .. } => "@hotlink",
		Stmt::GhostVar { .. } => "@ghost",
		Stmt::Block { .. } => "block",
		Stmt::Class { .. } => "class",
		Stmt::BreakStmt { .. } => "break",
		Stmt::ContinueStmt { .. } => "continue",
		Stmt::IfStmt { .. } => "if",
		Stmt::WhileStmt { .. } => "while",
		Stmt::ForStmt { .. } => "for",
		Stmt::Function { .. } => "func",
		Stmt::CmdFunction { .. } => "command function (<-)",
		Stmt::ReturnStmt { .. } => "return",
		Stmt::EntryFunction { .. } => "defi",
	}
}

fn expr_refs_ghost(expr: &Expr, is_ghost: &dyn Fn(&str) -> bool) -> bool {
	match expr {
		Expr::Variable { name, .. } => is_ghost(&name.lexeme),
		Expr::Assign { name, value, .. } => is_ghost(&name.lexeme) || expr_refs_ghost(value, is_ghost),
		Expr::Binary { left, right, .. } | Expr::Logical { left, right, .. } => {
			expr_refs_ghost(left, is_ghost) || expr_refs_ghost(right, is_ghost)
		}
		Expr::Call { callee, arguments, .. } => {
			expr_refs_ghost(callee, is_ghost) || arguments.iter().any(|a| expr_refs_ghost(a, is_ghost))
		}
		Expr::Get { object, .. } => expr_refs_ghost(object, is_ghost),
		Expr::Set { object, value, .. } => expr_refs_ghost(object, is_ghost) || expr_refs_ghost(value, is_ghost),
		Expr::Grouping { expression, .. } => expr_refs_ghost(expression, is_ghost),
		Expr::Unary { right, .. } => expr_refs_ghost(right, is_ghost),
		Expr::ListLiteral { items, .. } => items.iter().any(|i| expr_refs_ghost(i, is_ghost)),
		Expr::Index { object, index, .. } => expr_refs_ghost(object, is_ghost) || expr_refs_ghost(index, is_ghost),
		Expr::IndexSet { object, index, value, .. } => {
			expr_refs_ghost(object, is_ghost) || expr_refs_ghost(index, is_ghost) || expr_refs_ghost(value, is_ghost)
		}
		// Statements inside an anon function body go through compile_stmt, which does its own check
		Expr::Literal { .. } | Expr::This { .. } | Expr::Super { .. } | Expr::AnonFunction { .. } => false,
	}
}

impl Compiler {
	pub fn new() -> Self {
		Self::build(HashMap::new(), 0, false)
	}

	pub fn with_globals(existing_globals: HashMap<String, usize>, next_slot: usize, is_production: bool) -> Self {
		Self::build(existing_globals, next_slot, is_production)
	}

	fn build(global_slots: HashMap<String, usize>, next_slot: usize, is_production: bool) -> Self {
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
			scopes: vec![top_level],
			global_slots,
			next_global_slot: next_slot,
			loop_stack: vec![],
			is_production,
			ghost_frames: vec![HashMap::new()],
			hotlinks: HashMap::new(),
			global_aliased: HashSet::new(),
		}
	}

	fn current(&mut self) -> &mut FunctionScope {
		self.scopes.last_mut().expect("Scopes should never be empty")
	}

	fn begin_scope(&mut self) {
		self.current().scope_depth += 1;
		self.ghost_frames.push(HashMap::new());
	}

	fn end_scope(&mut self) {
		let depth = self.current().scope_depth;
		self.current().scope_depth -= 1;
		self.ghost_frames.pop();

		loop {
			let should_close = {
				let scope = self.current();
				match scope.locals.last() {
					Some(local) if local.depth == depth => Some(local.is_captured),
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

	fn is_ghost(&self, name: &str) -> bool {
		for frame in self.ghost_frames.iter().rev() {
			if let Some(&g) = frame.get(name) {
				return g;
			}
		}
		false
	}

	fn mark_name(&mut self, name: &str, is_ghost: bool) {
		self.ghost_frames.last_mut().unwrap().insert(name.to_string(), is_ghost);
	}

	fn stmt_refs_ghost(&self, stmt: &Stmt) -> bool {
		let g = |n: &str| self.is_ghost(n);
		match stmt {
			Stmt::Expression { expression } | Stmt::Print { expression } => expr_refs_ghost(expression, &g),
			Stmt::Var { initializer, .. } => expr_refs_ghost(initializer, &g),
			Stmt::WrafsVar { source, .. } | Stmt::HotlinkVar { source, .. } => g(&source.lexeme),
			Stmt::ReturnStmt { value: Some(v), .. } => expr_refs_ghost(v, &g),
			Stmt::IfStmt { predicate, .. } => expr_refs_ghost(predicate, &g),
			Stmt::WhileStmt { condition, .. } => expr_refs_ghost(condition, &g),
			Stmt::ForStmt { iterable, .. } => expr_refs_ghost(iterable, &g),
			_ => false,
		}
	}

	fn declare_local(&mut self, name: &str) -> usize {
		let depth = self.current().scope_depth;
		let slot = self.current().locals.len();
		let scope = self.scopes.len() - 1;
		// a reused slot must not inherit a stale hotlink from a variable that used to live there
		self.hotlinks.remove(&LinkTarget::Local { scope, slot });
		self.current().locals.push(LocalVar {
			name: name.to_string(),
			depth,
			is_captured: false,
			alias: None,
		});
		self.mark_name(name, false);
		slot
	}

	// Defines a new variable from the value on top of the stack (local or global).
	fn define_variable(&mut self, name: &str) -> LinkTarget {
		if self.current().scope_depth > 0 {
			let slot = self.declare_local(name);
			LinkTarget::Local { scope: self.scopes.len() - 1, slot }
		} else {
			// redefining a name that was a @wrafs alias must give it its own slot again
			if self.global_aliased.remove(name) {
				self.global_slots.remove(name);
			}
			let slot = self.resolve_or_create_global(name);
			self.hotlinks.remove(&LinkTarget::Global(slot));
			self.mark_name(name, false);
			self.current().chunk.emit(OpCode::DefineGlobal(slot));
			LinkTarget::Global(slot)
		}
	}

	// Looks a name up as seen from function scope `si`. None means "not a local or upvalue" (so: global).
	fn resolve_in_scope(&mut self, si: usize, name: &str) -> Option<VarLocation> {
		let found = self.scopes[si]
			.locals
			.iter()
			.enumerate()
			.rev()
			.find(|(_, l)| l.name == name)
			.map(|(i, l)| (i, l.alias));
		if let Some((slot, alias)) = found {
			return Some(match alias {
				Some(target) => self.locate_target(si, target),
				None => VarLocation::Local(slot),
			});
		}
		if si == 0 {
			return None;
		}
		if let Some(&idx) = self.scopes[si].upvalue_names.get(name) {
			return Some(VarLocation::Upvalue(idx));
		}
		match self.resolve_in_scope(si - 1, name)? {
			VarLocation::Local(slot) => {
				Some(VarLocation::Upvalue(self.add_upvalue(si, name, UpvalueSource::Local(slot))))
			}
			VarLocation::Upvalue(i) => {
				Some(VarLocation::Upvalue(self.add_upvalue(si, name, UpvalueSource::Upvalue(i))))
			}
			VarLocation::Global(s) => Some(VarLocation::Global(s)),
		}
	}

	// Where do I find the storage `target` from inside function scope `si`?
	fn locate_target(&mut self, si: usize, target: LinkTarget) -> VarLocation {
		match target {
			LinkTarget::Global(s) => VarLocation::Global(s),
			LinkTarget::Local { scope, slot } if scope == si => VarLocation::Local(slot),
			LinkTarget::Local { scope, slot } => {
				let key = format!("\0link:{}:{}", scope, slot);
				if let Some(&idx) = self.scopes[si].upvalue_names.get(&key) {
					return VarLocation::Upvalue(idx);
				}
				let source = match self.locate_target(si - 1, target) {
					VarLocation::Local(sl) => UpvalueSource::Local(sl),
					VarLocation::Upvalue(i) => UpvalueSource::Upvalue(i),
					VarLocation::Global(s) => return VarLocation::Global(s),
				};
				VarLocation::Upvalue(self.add_upvalue(si, &key, source))
			}
		}
	}

	fn target_of(&self, loc: VarLocation) -> LinkTarget {
		let cur = self.scopes.len() - 1;
		match loc {
			VarLocation::Global(s) => LinkTarget::Global(s),
			VarLocation::Local(slot) => LinkTarget::Local { scope: cur, slot },
			VarLocation::Upvalue(idx) => {
				let mut si = cur;
				let mut i = idx;
				loop {
					match self.scopes[si].upvalues[i] {
						UpvalueSource::Local(slot) => return LinkTarget::Local { scope: si - 1, slot },
						UpvalueSource::Upvalue(j) => {
							si -= 1;
							i = j;
						}
					}
				}
			}
		}
	}

	fn resolve_variable(&mut self, name: &str) -> VarLocation {
		let si = self.scopes.len() - 1;
		match self.resolve_in_scope(si, name) {
			Some(loc) => loc,
			None => VarLocation::Global(self.resolve_or_create_global(name)),
		}
	}

	// Like resolve_variable, but a global that was never declared is an error (for @wrafs / @hotlink sources).
	fn resolve_existing(&mut self, name: &str, line: usize, what: &str) -> Result<VarLocation, String> {
		let si = self.scopes.len() - 1;
		match self.resolve_in_scope(si, name) {
			Some(loc) => Ok(loc),
			None => match self.global_slots.get(name) {
				Some(&slot) => Ok(VarLocation::Global(slot)),
				None => Err(format!("Line {}: '{}' has not been declared, cannot {} to it", line, name, what)),
			},
		}
	}

	pub fn compile(mut self, stmts: &[Stmt]) -> Result<(Chunk, usize), String> {
		for stmt in stmts {
			self.compile_stmt(stmt)?;
		}
		let nil_index = self.scopes[0].chunk.add_constant(ConstantValue::Nil);
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

	fn emit_get_location(&mut self, loc: VarLocation) {
		let op = match loc {
			VarLocation::Local(slot) => OpCode::GetLocal(slot),
			VarLocation::Upvalue(index) => OpCode::GetUpvalue(index),
			VarLocation::Global(slot) => OpCode::GetGlobal(slot),
		};
		self.current().chunk.emit(op);
	}

	fn emit_set_location(&mut self, loc: VarLocation) {
		let op = match loc {
			VarLocation::Local(slot) => OpCode::SetLocal(slot),
			VarLocation::Upvalue(index) => OpCode::SetUpvalue(index),
			VarLocation::Global(slot) => OpCode::SetGlobal(slot),
		};
		self.current().chunk.emit(op);
	}

	fn emit_get_variable(&mut self, name: &str) {
		let loc = self.resolve_variable(name);
		self.emit_get_location(loc);
	}

	// Set* leaves the value on the stack, so a hotlinked write is just "set me, then set my source (and its source...)".
	fn emit_set_variable(&mut self, name: &str) {
		let loc = self.resolve_variable(name);
		self.emit_set_location(loc);

		let cur = self.scopes.len() - 1;
		let mut target = self.target_of(loc);
		while let Some(source) = self.hotlinks.get(&target).copied() {
			let source_loc = self.locate_target(cur, source);
			self.emit_set_location(source_loc);
			target = source;
		}
	}

	fn link_chain_reaches(&self, from: LinkTarget, to: LinkTarget) -> bool {
		let mut t = from;
		loop {
			if t == to {
				return true;
			}
			match self.hotlinks.get(&t) {
				Some(&next) => t = next,
				None => return false,
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

	// Emits the pops needed to unwind to `keep` locals without forgetting them at compile time
	// (code after the break/continue in the same block still sees those locals).
	fn emit_pops_to(&mut self, keep: usize) {
		let total = self.current().locals.len();
		for i in (keep..total).rev() {
			let captured = self.current().locals[i].is_captured;
			if captured {
				self.current().chunk.emit(OpCode::CloseUpvalue(i));
			} else {
				self.current().chunk.emit(OpCode::Pop);
			}
		}
	}

	fn compile_stmt(&mut self, stmt: &Stmt) -> Result<(), String> {
		// Production builds strip @ghost declarations and everything that touches them.
		if self.is_production {
			if self.stmt_refs_ghost(stmt) {
				// whatever this statement would have declared now depends on a ghost, so it is ghost too
				match stmt {
					Stmt::Var { name, .. } | Stmt::WrafsVar { name, .. } | Stmt::HotlinkVar { name, .. } => {
						self.mark_name(&name.lexeme, true);
					}
					_ => {}
				}
				return Ok(());
			}
			if let Stmt::GhostVar { name, .. } = stmt {
				self.mark_name(&name.lexeme, true);
				return Ok(());
			}
		}

		match stmt {
			Stmt::Var { name, initializer } => {
				self.compile_expr(initializer)?;
				self.define_variable(&name.lexeme);
			}
			Stmt::GhostVar { name, initializer } => {
				// development build: a ghost is just a normal variable
				self.compile_expr(initializer)?;
				self.define_variable(&name.lexeme);
			}
			Stmt::WrafsVar { id: _, name, source } => {
				let loc = self.resolve_existing(&source.lexeme, source.line_number, "@wrafs")?;
				let target = self.target_of(loc);
				if self.current().scope_depth > 0 {
					// dummy slot keeps the stack aligned; the name itself just points at `target`
					let nil = self.current().chunk.add_constant(ConstantValue::Nil);
					self.current().chunk.emit(OpCode::Const(nil));
					let slot = self.declare_local(&name.lexeme);
					self.current().locals[slot].alias = Some(target);
				} else if let LinkTarget::Global(slot) = target {
					self.global_slots.insert(name.lexeme.clone(), slot);
					self.global_aliased.insert(name.lexeme.clone());
					self.mark_name(&name.lexeme, false);
				} else {
					return Err(format!("Line {}: internal error, global @wrafs of a local", name.line_number));
				}
			}
			Stmt::HotlinkVar { id: _, name, source } => {
				let loc = self.resolve_existing(&source.lexeme, source.line_number, "@hotlink")?;
				let target = self.target_of(loc);
				// the linked variable starts as a copy of the source...
				self.emit_get_location(loc);
				let linked = self.define_variable(&name.lexeme);
				// ...and from then on, writing it also writes the source (not the other way round)
				if self.link_chain_reaches(target, linked) {
					return Err(format!(
						"Line {}: '{}' cannot be hotlinked to '{}' (it would link back to itself)",
						name.line_number, name.lexeme, source.lexeme
					));
				}
				self.hotlinks.insert(linked, target);
			}
			Stmt::Print { expression } => {
				self.compile_expr(expression)?;
				self.current().chunk.emit(OpCode::Print);
			}
			Stmt::Expression { expression } => {
				self.compile_expr(expression)?;
				self.current().chunk.emit(OpCode::Pop);
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
				let after_target = self.current().chunk.next_index();
				self.current().chunk.patch_jump(jump_past_else, after_target);
			}
			Stmt::WhileStmt { condition, body } => {
				let loop_start = self.current().chunk.next_index();
				self.compile_expr(condition)?;
				let jump_to_end = self.current().chunk.emit_jump_placeholder(OpCode::JumpIfFalse);
				let locals_len = self.current().locals.len();
				self.loop_stack.push(LoopContext {
					continue_target: Some(loop_start),
					continue_jumps: vec![],
					break_jumps: vec![],
					locals_len,
				});
				self.compile_stmt(body.as_ref())?;
				self.current().chunk.emit(OpCode::Jump(loop_start));
				let end_target = self.current().chunk.next_index();
				self.current().chunk.patch_jump(jump_to_end, end_target);
				let ctx = self.loop_stack.pop().expect("loop stack should not be empty");
				for break_index in ctx.break_jumps {
					self.current().chunk.patch_jump(break_index, end_target);
				}
			}
			Stmt::ForStmt { variable, iterable, body } => {
				self.begin_scope();
				self.compile_expr(iterable)?;
				let range_slot = self.declare_local("__range__");
				let zero_index = self.current().chunk.add_constant(ConstantValue::Number(0.0));
				self.current().chunk.emit(OpCode::Const(zero_index));

				let counter_slot = self.declare_local("__counter__");
				let nil_index = self.current().chunk.add_constant(ConstantValue::Nil);
				self.current().chunk.emit(OpCode::Const(nil_index));

				let var_slot = self.declare_local(&variable.lexeme);
				let iter_start = self.current().chunk.next_index();
				let jump_if_done = self.current().chunk.emit_jump_placeholder(|t| OpCode::ForIterStart(t, range_slot, counter_slot, var_slot));
				let locals_len = self.current().locals.len();
				self.loop_stack.push(LoopContext {
					continue_target: None,
					continue_jumps: vec![],
					break_jumps: vec![],
					locals_len,
				});
				self.compile_stmt(body.as_ref())?;
				let ctx = self.loop_stack.pop().unwrap();

				// `continue` must land on ForIterNext, otherwise the counter never advances
				let next_index = self.current().chunk.next_index();
				for continue_index in ctx.continue_jumps {
					self.current().chunk.patch_jump(continue_index, next_index);
				}
				self.current().chunk.emit(OpCode::ForIterNext(iter_start, counter_slot));
				let end_target = self.current().chunk.next_index();

				self.current().chunk.patch_jump(jump_if_done, end_target);
				for break_index in ctx.break_jumps {
					self.current().chunk.patch_jump(break_index, end_target);
				}
				self.end_scope();
			}
			Stmt::BreakStmt { keyword: _ } => {
				let keep = match self.loop_stack.last() {
					Some(ctx) => ctx.locals_len,
					None => return Err("'break' is not allowed outside of a loop".to_string()),
				};
				self.emit_pops_to(keep);
				let jump_index = self.current().chunk.emit_jump_placeholder(OpCode::Jump);
				self.loop_stack.last_mut().unwrap().break_jumps.push(jump_index);
			}
			Stmt::ContinueStmt { keyword: _ } => {
				let (keep, target) = match self.loop_stack.last() {
					Some(ctx) => (ctx.locals_len, ctx.continue_target),
					None => return Err("'continue' is not allowed outside of a loop".to_string()),
				};
				self.emit_pops_to(keep);
				match target {
					Some(t) => self.current().chunk.emit(OpCode::Jump(t)),
					None => {
						let jump_index = self.current().chunk.emit_jump_placeholder(OpCode::Jump);
						self.loop_stack.last_mut().unwrap().continue_jumps.push(jump_index);
					}
				}
			}
			Stmt::ReturnStmt { keyword: _, value } => {
				if let Some(v) = value {
					self.compile_expr(v)?;
				} else {
					let index = self.current().chunk.add_constant(ConstantValue::Nil);
					self.current().chunk.emit(OpCode::Const(index));
				}
				self.current().chunk.emit(OpCode::Return);
			}
			Stmt::Function { name, params, body } => {
				self.compile_function(&name.lexeme, &params.iter().map(|t| t.lexeme.clone()).collect::<Vec<_>>(), &body.iter().map(|b| b.as_ref()).collect::<Vec<_>>(),)?;
				self.define_variable(&name.lexeme);
			}
			other => {
				return Err(format!("VM Compiler: '{}' is not supported yet", stmt_kind(other)));
			}
		}
		Ok(())
	}

	fn compile_expr(&mut self, expr: &Expr) -> Result<(), String> {
		match expr {
			Expr::Literal { id: _, value } => {
				let cv = Self::literal_to_constant(value)?;
				let index = self.current().chunk.add_constant(cv);
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
					TokenType::Plus => self.current().chunk.emit(OpCode::Add),
					TokenType::Minus => self.current().chunk.emit(OpCode::Sub),
					TokenType::Star => self.current().chunk.emit(OpCode::Mul),
					TokenType::Slash => self.current().chunk.emit(OpCode::Div),
					TokenType::Percent => self.current().chunk.emit(OpCode::Mod),
					TokenType::EqualEqual => self.current().chunk.emit(OpCode::Equal),
					TokenType::BangEqual => self.current().chunk.emit(OpCode::NotEqual),
					TokenType::Greater => self.current().chunk.emit(OpCode::Greater),
					TokenType::GreaterEqual => self.current().chunk.emit(OpCode::GreaterEqual),
					TokenType::Less => self.current().chunk.emit(OpCode::Less),
					TokenType::LessEqual => self.current().chunk.emit(OpCode::LessEqual),
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
				// JumpIfFalse pops its operand, so Dup first to keep the left value as the result when we short-circuit
				match operator.token_type {
					TokenType::And => {
						// left; Dup; JumpIfFalse(end); Pop; right; end:
						self.compile_expr(left)?;
						self.current().chunk.emit(OpCode::Dup);
						let short_circuit = self.current().chunk.emit_jump_placeholder(OpCode::JumpIfFalse);
						self.current().chunk.emit(OpCode::Pop);
						self.compile_expr(right)?;
						let after = self.current().chunk.next_index();
						self.current().chunk.patch_jump(short_circuit, after);
					}
					TokenType::Or => {
						// left; Dup; JumpIfFalse(rhs); Jump(end); rhs: Pop; right; end:
						self.compile_expr(left)?;
						self.current().chunk.emit(OpCode::Dup);
						let check_false = self.current().chunk.emit_jump_placeholder(OpCode::JumpIfFalse);
						let skip_right = self.current().chunk.emit_jump_placeholder(OpCode::Jump);
						let right_start = self.current().chunk.next_index();
						self.current().chunk.patch_jump(check_false, right_start);
						self.current().chunk.emit(OpCode::Pop);
						self.compile_expr(right)?;
						let after = self.current().chunk.next_index();
						self.current().chunk.patch_jump(skip_right, after);
					}
					_ => return Err("Invalid logical operator".to_string()),
				}
			}
			other => {
				return Err(format!("VM Compiler: unsupported expression: {}", other.to_string()))
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
		self.ghost_frames.push(HashMap::new());
		// break/continue can't jump out of a function into an enclosing loop
		let saved_loops = std::mem::take(&mut self.loop_stack);

		for param in params {
			self.declare_local(param);
		}
		for stmt in body {
			self.compile_stmt(stmt)?;
		}
		let nil_index = self.current().chunk.add_constant(ConstantValue::Nil);
		self.current().chunk.emit(OpCode::Const(nil_index));
		self.current().chunk.emit(OpCode::Return);

		self.loop_stack = saved_loops;
		self.ghost_frames.pop();
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