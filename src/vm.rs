use crate::bytecode::{Chunk, Closure, OpCode, UpvalueSource, VMFunction, ConstantValue};
use crate::compiler::Compiler;
use crate::expr::LiteralValue;
use crate::parser::Parser;
use crate::scanner::Scanner;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

struct CallFrame {
	closure: Rc<Closure>,
	ip: usize,
	stack_base: usize,
}

impl CallFrame {
	fn chunk(&self) -> &Chunk {
		&self.closure.function.chunk
	}
}

struct OpenUpvalue {
	stack_index: usize,
	cell: Rc<RefCell<LiteralValue>>,
}

pub struct PyxisVM {
	globals: Vec<LiteralValue>,
	stack: Vec<LiteralValue>,
	open_upvalues: Vec<OpenUpvalue>,
	native_slots: HashMap<String, usize>,
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

impl PyxisVM {
	pub fn new() -> Self {
		let mut vm = Self {
			globals: vec![],
			stack: vec![],
			open_upvalues: vec![],
			native_slots: HashMap::new(),
		};

		vm.register_native("range", 1, |args| {
			match &args[0] {
				LiteralValue::Number(n) => Ok(LiteralValue::Range(0, *n as i64)),
				other => Err(format!("range() expects a Number, got {}", other.to_type())),
			}
		});
		vm.register_native("len", 1, |args| {
			match &args[0] {
				LiteralValue::List { items } => Ok(LiteralValue::Number(items.borrow().len() as f64)),
				LiteralValue::StringValue(s) => Ok(LiteralValue::Number(s.len() as f64)),
				other => Err(format!("len() expects a array or string, got {}", other.to_type())),
			}
		});
		vm.register_native("clock", 0, |_args| {
			let now = std::time::SystemTime::now()
				.duration_since(std::time::SystemTime::UNIX_EPOCH)
				.expect("Could not get system time")
				.as_millis();
			Ok(LiteralValue::Number(now as f64 / 1000.0))	
		});
		vm
	}

	pub fn load(&mut self, source: &str) -> Result<(), String> {
		let mut scanner = Scanner::new(source);
		let tokens = scanner.scanTokens()?;
		let mut parser = Parser::new(tokens);
		let stmts = parser.parse()?;

		let compiler = Compiler::with_globals(self.native_slots.clone(), self.globals.len());
		let (chunk, global_count) = compiler.compile(&stmts)?;

		if self.globals.len() < global_count {
			self.globals.resize(global_count, LiteralValue::Nil);
		}
		
		let script_fn = Rc::new(VMFunction {
			name: "<script>".to_string(),
			arity: 0,
			chunk,
			upvalue_sources: vec![],
		});
		let script_closure = Rc::new(Closure {
			function: script_fn,
			upvalues: vec![],
		});
		self.run(script_closure)
	}

	pub fn register_native(&mut self, name: &str, arity: usize, f: impl Fn(&[LiteralValue]) -> Result<LiteralValue, String> + 'static,) {
		let native = LiteralValue::VMNative {
			name: name.to_string(),
			arity,
			fun: Rc::new(f),
		};

		let slot = self.globals.len();
		self.globals.push(native);
		self.native_slots.insert(name.to_string(), slot);
	}

	fn run(&mut self, initial_closure: Rc<Closure>) -> Result<(), String> {
		let mut frames: Vec<CallFrame> = vec![CallFrame {
			closure: initial_closure,
			ip: 0,
			stack_base: 0,
		}];

		macro_rules! frame {
			() => {
				frames.last_mut().unwrap()
			};
		}

		loop {
			let instr = {
				let f  = frames.last().unwrap();
				if f.ip >= f.chunk().code.len() {
					break;
				}
				f.chunk().code[f.ip].clone()
			};
			frame!().ip += 1;
			match instr {
				OpCode::Const(index) => {
					let v = frames.last().unwrap().chunk().constants[index].clone();
					self.stack.push(constant_to_literal(v));
				}
				OpCode::Add => self.binary_numeric_op(|a, b| a + b)?,
				OpCode::Sub => self.binary_numeric_op(|a, b| a - b)?,
				OpCode::Mul => self.binary_numeric_op(|a, b| a * b)?,
				OpCode::Div => self.binary_numeric_op(|a, b| a / b)?,
				OpCode::Mod => self.binary_numeric_op(|a, b| a % b)?,
				OpCode::Negate => {
					let v = self.stack.pop().expect("Stack underflow on negate");
					match v {
						LiteralValue::Number(n) => self.stack.push(LiteralValue::Number(-n)),
						other => {
							return Err(format!("Cannot negate a {}", other.to_type()));
						}
					}
				}
				OpCode::Not => {
					let v = self.stack.pop().unwrap();
					self.stack.push(v.is_falsy());
				}
				OpCode::GetGlobal(slot) => {
					self.stack.push(self.globals[slot].clone());
				}
				OpCode::SetGlobal(slot) => {
					let v = self.stack.last().unwrap().clone();
					self.globals[slot] = v;
				}
				OpCode::DefineGlobal(slot) => {
					let v = self.stack.pop().unwrap();
					self.globals[slot] = v;
				}
				OpCode::GetLocal(slot) => {
					let base = frame!().stack_base;
					let v = self.stack[base + slot].clone();
					self.stack.push(v);
				}
				OpCode::SetLocal(slot) => {
					let base = frame!().stack_base;
					let v = self.stack.last().unwrap().clone();
					self.stack[base + slot] = v;
				}
				OpCode::GetUpvalue(index) => {
					let v = frames.last().unwrap().closure.upvalues[index].borrow().clone();
					self.stack.push(v);
				}
				OpCode::SetUpvalue(index) => {
					let v = self.stack.last().unwrap().clone();
					*frames.last().unwrap().closure.upvalues[index].borrow_mut() = v;
				}
				OpCode::Print => {
					let v = self.stack.pop().unwrap();
					println!("{}", v.to_string());
				}
				OpCode::Pop => {
					self.stack.pop();
				}
				OpCode::JumpIfFalse(target) => {
					let v = self.stack.pop().expect("Stack underflow on JumpIfFalse");
					if v.is_truthy() == LiteralValue::False {
						frame!().ip = target;
					}
				}
				OpCode::Jump(target) => {
					frame!().ip = target;
				}
				OpCode::Equal => self.binary_compare_op(|a, b| a == b)?,
				OpCode::NotEqual => self.binary_compare_op(|a, b| a != b)?,
				OpCode::Greater => self.binary_numeric_compare(|a, b| a > b)?,
				OpCode::GreaterEqual => self.binary_numeric_compare(|a, b| a >= b)?,
				OpCode::Less => self.binary_numeric_compare(|a, b| a < b)?,
				OpCode::LessEqual => self.binary_numeric_compare(|a, b| a <= b)?,
				OpCode::Closure(fn_index) => {
					let fn_references = frames.last().unwrap().chunk().child_functions[fn_index].clone();

					let mut upvalues = vec![];
					let sources = fn_references.upvalue_sources.clone();
					for source in &sources {
						let cell = match source {
							UpvalueSource::Local(slot) => {
								let abs_slot = frame!().stack_base + slot;
								self.capture_local(abs_slot)
							}
							UpvalueSource::Upvalue(index) => {
								frames.last().unwrap().closure.upvalues[*index].clone()
							}
						};
						upvalues.push(cell);
					}
					let closure = LiteralValue::VMClosure(Rc::new(Closure {
						function: fn_references,
						upvalues,
					}));
					self.stack.push(closure);
				}
				OpCode::CloseUpvalue(slot) => {
					let abs_slot = frame!().stack_base + slot;
					self.close_upvalue(abs_slot);
					self.stack.pop();
				}
				OpCode::Call(argument_count) => {
					let callee_index = self.stack.len() - argument_count - 1;
					let callee = self.stack[callee_index].clone();

					match callee {
						LiteralValue::VMClosure(closure) => {
							if argument_count != closure.function.arity {
								return Err(format!("Expected {} arguments but got {}", closure.function.arity, argument_count));
							}
							let new_base = callee_index + 1;
							frames.push(CallFrame {
								closure,
								ip: 0,
								stack_base: new_base,
							});
						}
						LiteralValue::VMNative { name, arity, fun } => {
							if argument_count != arity {
								return Err(format!(
									"{}() expected {} arguments, got {}",
									name, arity, argument_count
								));
							}
							let args: Vec<LiteralValue> = self.stack[callee_index + 1..].to_vec();
							let result = (fun)(&args)?;

							self.stack.truncate(callee_index);
							self.stack.push(result);
						}
						other => {
							return Err(format!("{} is not callables", other.to_type()));
						}
					}
				}
				OpCode::ForIterStart(jump_if_done, range_slot, counter_slot, var_slot) => {
					let base = frame!().stack_base;
					let range = self.stack[base + range_slot].clone();
					let counter = match &self.stack[base + counter_slot] {
						LiteralValue::Number(n) => *n as i64,
						other => return Err(format!("For loop counter must be a number, got {}", other.to_type())),
					};
					match range {
						LiteralValue::Range(start, end) => {
							if start + counter >= end {
								frame!().ip = jump_if_done;
							} else {
								self.stack[base + var_slot] = LiteralValue::Number((start + counter) as f64);
							}
						}
						other => return Err(format!("Expected a range to iterate, got {}", other.to_type())),
					}
				}
				OpCode::ForIterNext(jump_back, counter_slot) => {
					let base = frame!().stack_base;
					match &self.stack[base + counter_slot] {
						LiteralValue::Number(n) => {
							let new_value = n + 1.0;
							self.stack[base + counter_slot] = LiteralValue::Number(new_value);
						}
						other => return Err(format!("For loop counter corrupted: {}", other.to_type())),
					}
					frame!().ip = jump_back;
					continue;
				}
				OpCode::Return => {
					let return_value = self.stack.pop().unwrap();
					let frame_base = frame!().stack_base;

					self.close_upvalues_above(frame_base);

					let popped_frame = frames.pop().unwrap();
					if frames.is_empty() {
						break;
					}
					self.stack.truncate(popped_frame.stack_base - 1);
					self.stack.push(return_value);
				}
			}
		}
		Ok(())
	}

	fn capture_local(&mut self, abs_slot:usize) -> Rc<RefCell<LiteralValue>> {
		for open in &self.open_upvalues {
			if open.stack_index == abs_slot {
				return open.cell.clone();
			}
		}

		let current_value = self.stack[abs_slot].clone();
		let cell = Rc::new(RefCell::new(current_value));
		self.open_upvalues.push(OpenUpvalue {
			stack_index: abs_slot,
			cell: cell.clone(),
		});
		cell
	}

	fn close_upvalue(&mut self, abs_slot: usize) {
		self.open_upvalues.retain(|open| {
			if open.stack_index == abs_slot {
				*open.cell.borrow_mut() = self.stack[abs_slot].clone();
				false
			} else {
				true
			}
		});
	}

	fn close_upvalues_above(&mut self, min_slot: usize) {
		let stack = &self.stack;
		self.open_upvalues.retain(|open| {
			if open.stack_index >= min_slot {
				*open.cell.borrow_mut() = stack[open.stack_index].clone();
				false
			} else {
				true
			}
		});
	}

	fn binary_numeric_op(&mut self, op: impl Fn(f64, f64) -> f64) -> Result<(), String> {
		let b = self.stack.pop().expect("Stack underflow (rhs)");
		let a = self.stack.pop().expect("Stack underflow (lhs)");
		match (a, b) {
			(LiteralValue::Number(x), LiteralValue::Number(y)) => {
				self.stack.push(LiteralValue::Number(op(x, y)));
				Ok(())
			}
			(a, b) => Err(format!(
				"Arithmetic not supported for {}and {}",
				a.to_type(),
				b.to_type()
			)),
		}
	}

	fn binary_compare_op(&mut self, op: impl Fn(&LiteralValue, &LiteralValue) -> bool) -> Result<(), String> {
		let b = self.stack.pop().expect("Stack underflow (rhs)");
		let a = self.stack.pop().expect("Stack underflow (lhs)");

		self.stack.push(LiteralValue::from_bool(op(&a, &b)));
		Ok(())
	}

	fn binary_numeric_compare(&mut self, op: impl Fn(f64, f64) -> bool) -> Result<(), String> {
		let b = self.stack.pop().expect("Stack underflow (rhs)");
		let a = self.stack.pop().expect("Stack underflow (lhs)");

		match (a, b) {
			(LiteralValue::Number(x), LiteralValue::Number(y)) => {
				self.stack.push(LiteralValue::from_bool(op(x, y)));
				Ok(())
			}
			(a, b) => Err(format!(
				"Comparison not supported for {} and {}",
				a.to_type(),
				b.to_type()
			)),
		}
	}
}