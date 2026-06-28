use crate::environment::Environment;
use crate::expr::{CallableImpl, LiteralValue, PyxisFunctionImpl, NativeFunctionImpl, stmtReferencesGhost};
use crate::scanner::Token;
use crate::stmt::Stmt;
use std::collections::HashMap;
use std::process::Command;
use std::rc::Rc;

pub struct Interpreter {
    pub specials: HashMap<String, LiteralValue>,
    pub environment: Environment,
	pub ghost_names: std::collections::HashSet<String>,
}

impl Interpreter {
    pub fn new(is_production: bool) -> Self {
	    let environment = Environment::new(HashMap::new(), is_production);
		
	    let range_fn = LiteralValue::Callable(
	        CallableImpl::NativeFunction(NativeFunctionImpl {
	            name: "range".to_string(),
	            arity: 1,
	            fun: Rc::new(|args: &Vec<LiteralValue>| match &args[0] {
	                LiteralValue::Number(end) => LiteralValue::Range(0, *end as i64),
	                _ => LiteralValue::Nil,
	            }),
	        }),
	    );

		let len_fn = LiteralValue::Callable(
			CallableImpl::NativeFunction(NativeFunctionImpl {
				name: "len".to_string(),
				arity: 1,
				fun: Rc::new(|args: &Vec<LiteralValue>| match &args[0] {
					LiteralValue::List { items } => {
						LiteralValue::Number(items.borrow().len() as f64)
					}
					LiteralValue::StringValue(s) => LiteralValue::Number(s.len() as f64),
					_ => LiteralValue::Nil,
				}),
			}),
		);
	
		environment.define("len".to_string(), len_fn);
	    environment.define("range".to_string(), range_fn);
	
	    Self {
	        specials: HashMap::new(),
	        environment,
			ghost_names: std::collections::HashSet::new(),
	    }
	}

    pub fn resolve(&mut self, locals: HashMap<usize, usize>) {
        self.environment.resolve(locals);
    }

	#[allow(non_snake_case)]
    pub fn withEnv(env: Environment) -> Self {
        Self {
            specials: HashMap::new(),
            environment: env,
			ghost_names: std::collections::HashSet::new(),
        }
    }

    #[allow(dead_code)]
	#[allow(non_snake_case)]
    pub fn forAnon(parent: Environment, is_production: bool) -> Self {
        let env = parent.enclose();
        Self {
            specials: HashMap::new(),
            environment: env,
			ghost_names: std::collections::HashSet::new(),
        }
    }

	#[allow(non_snake_case)]
    pub fn interpret(&mut self, stmts: Vec<&Stmt>) -> Result<(), String> {
        for stmt in stmts {
			if self.environment.is_production && stmtReferencesGhost(stmt, &self.ghost_names) {
				continue;
			}
            match stmt {
                Stmt::Expression { expression } => {
                    expression.evaluate(self.environment.clone())?;
                }
                Stmt::Print { expression } => {
                    let value = expression.evaluate(self.environment.clone())?;
                    println!("{}", value.to_string());
                }
                Stmt::Var { name, initializer } => {
                    let value = initializer.evaluate(self.environment.clone())?;
                    self.environment.define(name.lexeme.clone(), value);
                }
                Stmt::WrafsVar { id, name, source } => {
                	let distance = self.environment.getDistance(*id);
                	let slot = self.environment.getSlot(&source.lexeme, distance);
                	
                	match slot {
                		Some(existing_slot) => {
                			self.environment.defineAliased(name.lexeme.clone(), existing_slot);
                		} 
                		None => {
                			return Err(format!(
                				"Line {}: '{}' has not been declared, cannot @wrafs to it",
                				source.line_number, source.lexeme
                			));
                		}
                	}
                }
				Stmt::HotlinkVar { id, name, source } => {
					let distance = self.environment.getDistance(*id);
					let source_slot = self.environment.getSlot(&source.lexeme, distance);

					match source_slot {
						Some(source_slot) => {
							let current_value = source_slot.borrow().value.clone();

							self.environment.define(name.lexeme.clone(), current_value);
							let new_slot = self.environment.getSlot(&name.lexeme, Some(0)).expect("Defined variable should exist at a distance '[0]'");
							Environment::registerHotlink(&source_slot, new_slot);
						}
						None => {
							return Err(format!("Line {}: '{}' has not been declared, cannot @hotlink to i", source.line_number, source.lexeme));
						}
					}
				}
				Stmt::GhostVar { name, initializer } => {
					self.ghost_names.insert(name.lexeme.clone());
					if !self.environment.is_production {
						let value = initializer.evaluate(self.environment.clone())?;
						self.environment.define(name.lexeme.clone(), value);
					}
				}
                Stmt::Block { statements } => {
                    let new_environment = self.environment.enclose();

                    //     Environment::new();
                    // new_environment.enclosing = Some(Box::new(self.environment.clone()));
                    let old_environment = self.environment.clone();
                    self.environment = new_environment;
                    let block_result =
                        self.interpret((*statements).iter().map(|b| b.as_ref()).collect());
                    self.environment = old_environment;
                    // self.environment = self.environment.enclosing.unwrap();
                    block_result?;
                }
                Stmt::Class {
                    name,
                    methods,
                    superclass,
                } => {
                    let mut methodsMap = HashMap::new();

                    // Insert the methods of the superclass into the methods of this class
                    let superclassValue;
                    if let Some(superclass) = superclass {
                        let superclass = superclass.evaluate(self.environment.clone())?;
                        if let LiteralValue::PyxisClass { .. } = superclass {
                            superclassValue = Some(Box::new(superclass));
                        } else {
                            return Err(format!(
                                "Superclass must be a class, not {}",
                                superclass.to_type()
                            ));
                        }
                    } else {
                        superclassValue = None;
                    }

                    self.environment
                        .define(name.lexeme.clone(), LiteralValue::Nil);

                    self.environment = self.environment.enclose();
                    if let Some(sc) = superclassValue.clone() {
                        self.environment.define("super".to_string(), *sc);
                    }

                    for method in methods {
                        if let Stmt::Function {
                            name,
                            params: _,
                            body: _,
                        } = method.as_ref()
                        {
                            let function = self.makeFunction(method);
                            methodsMap.insert(name.lexeme.clone(), function);
                        } else {
                            panic!(
                                "Something that was not a function was in the methods of a class"
                            );
                        }
                    }

                    let klass = LiteralValue::PyxisClass {
                        name: name.lexeme.clone(),
                        methods: methodsMap,
                        superclass: superclassValue,
                    };

                    if !self.environment.assignGlobal(&name.lexeme, klass) {
                        return Err(format!("Class definition failed for {}", name.lexeme));
                    }

                    self.environment = *self.environment.enclosing.clone().unwrap();
                }
                Stmt::IfStmt {
                    predicate,
                    then,
                    els,
                } => {
                    let truth_value = predicate.evaluate(self.environment.clone())?;
                    if truth_value.is_truthy() == LiteralValue::True {
                        let statements = vec![then.as_ref()];
                        self.interpret(statements)?;
                    } else if let Some(els_stmt) = els {
                        let statements = vec![els_stmt.as_ref()];
                        self.interpret(statements)?;
                    }
                }
                Stmt::WhileStmt { condition, body } => {
                    let mut flag = condition.evaluate(self.environment.clone())?;
                    while flag.is_truthy() == LiteralValue::True {
                        let statements = vec![body.as_ref()];
                        self.interpret(statements)?;
                        if self.specials.contains_key("return") {
							break;
						}
						if self.specials.remove("break").is_some() {
							break;
						}
						self.specials.remove("continue");
						flag = condition.evaluate(self.environment.clone())?;
                    }
                }
				Stmt::ForStmt {
				    variable,
				    iterable,
				    body,
				} => {
				    let iterable_value = iterable.evaluate(self.environment.clone())?;
				
				    match iterable_value {
				        LiteralValue::Range(start, end) => {
				            for i in start..end {
				                self.environment.define(
				                    variable.lexeme.clone(),
				                    LiteralValue::Number(i as f64),
				                );
							
				                let statements = vec![body.as_ref()];
				                self.interpret(statements)?;

								if self.specials.contains_key("return") {
									break;
								}
								if self.specials.remove("break").is_some() {
									break;
								}
								self.specials.remove("continue");
				            }
				        }
					
				        _ => {
				            return Err(format!(
				                "Expected iterable in for loop, got {}",
				                iterable_value.to_type()
				            ));
				        }
				    }
				}
				Stmt::BreakStmt { keyword: _ } => {
					self.specials.insert("break".to_string(), LiteralValue::Nil);
				}
				Stmt::ContinueStmt { keyword: _ } => {
					self.specials.insert("continue".to_string(), LiteralValue::Nil);
				}
                Stmt::Function {
                    name,
                    params: _,
                    body: _,
                } => {
                    let callable = self.makeFunction(stmt);
                    let fun = LiteralValue::Callable(CallableImpl::PyxisFunction(callable));
                    self.environment.define(name.lexeme.clone(), fun);
                }
				Stmt::EntryFunction {
					name,
					params: _,
					body: _,
				} => {
					let callable = self.makeEntryFunction(stmt);
					let fun = LiteralValue::Callable(CallableImpl::PyxisFunction(callable));

					self.environment.define(name.lexeme.clone(), fun.clone());
					if name.lexeme == "main" {
						self.specials.insert("__entry__".to_string(), fun);
					}
				}
                Stmt::CmdFunction { name, cmd } => {
                    // Return a callable that runs a shell command, captures the stdout and returns
                    // it in a String

                    let cmd = cmd.clone();
                    let local_fn = move |_args: &Vec<LiteralValue>| {
                        let cmd = cmd.clone();
                        let parts = cmd.split(" ").collect::<Vec<&str>>();
                        let mut command = Command::new(parts[0].replace("\"", ""));
                        for part in parts[1..].iter() {
                            command.arg(part.replace("\"", ""));
                        }
                        let output = command.output().expect("Failed to run command");


                        return LiteralValue::StringValue(
                            std::str::from_utf8(output.stdout.as_slice())
                                .unwrap()
                                .to_string(),
                        );
                    };

                    let fun_val =
                        LiteralValue::Callable(CallableImpl::NativeFunction(NativeFunctionImpl {
                            name: name.lexeme.clone(),
                            arity: 0,
                            fun: Rc::new(local_fn),
                        }));
                    self.environment.define(name.lexeme.clone(), fun_val);
                }
                Stmt::ReturnStmt { keyword: _, value } => {
                    let eval_val;
                    if let Some(value) = value {
                        eval_val = value.evaluate(self.environment.clone())?;
                    } else {
                        eval_val = LiteralValue::Nil;
                    }
                    self.specials.insert("return".to_string(), eval_val);
                }
            };
			if self.specials.contains_key("return") || self.specials.contains_key("break") || self.specials.contains_key("continue") {
				break;
			}
        }

        Ok(())
    }

	#[allow(non_snake_case)]
    fn makeFunction(&self, fn_stmt: &Stmt) -> PyxisFunctionImpl {
        if let Stmt::Function { name, params, body } = fn_stmt {
            let arity = params.len();
            let params: Vec<Token> = params.iter().map(|t| (*t).clone()).collect();
            let body: Vec<Box<Stmt>> = body.iter().map(|b| (*b).clone()).collect();
            let name_clone = name.lexeme.clone();

            // TODO: Don't clone the whole environment, just the captured variables
            let parent_env = self.environment.clone();

            let callable_impl = PyxisFunctionImpl {
                name: name_clone,
                arity,
                parent_env,
                params,
                body,
            };

            callable_impl
        } else {
            panic!("Tried to make a function from a non-function statement");
        }
    }

	#[allow(non_snake_case)]
	fn makeEntryFunction(&self, fn_stmt: &Stmt) -> PyxisFunctionImpl {
		if let Stmt::EntryFunction { name, params, body } = fn_stmt {
			let arity = params.len();
			let params: Vec<Token> = params.iter().map(|t| (*t).clone()).collect();
			let body: Vec<Box<Stmt>> = body.iter().map(|b| (*b).clone()).collect();
			let name_clone = name.lexeme.clone();
			let parent_env = self.environment.clone();

			PyxisFunctionImpl {
				name: name_clone,
				arity,
				parent_env,
				params,
				body,
			}
		} else {
			panic!("Tried to make a function from a non-entry-function statement")
		}
	}
}