use crate::expr::Expr;
use crate::scanner::Token;
use crate::stmt::Stmt;
use std::collections::HashMap;

#[derive(Copy, Clone, PartialEq)]
enum FunctionType {
    None,
    Function,
    Method,
}

#[allow(dead_code)]
pub struct Resolver {
    scopes: Vec<HashMap<String, bool>>,
    current_function: FunctionType,
	loop_depth: usize,
    locals: HashMap<usize, usize>,
}

impl Resolver {
    pub fn new() -> Self {
        Self {
            scopes: vec![],
            current_function: FunctionType::None,
			loop_depth: 0,
            locals: HashMap::new(),
        }
    }

	#[allow(non_snake_case)]
    fn resolveInternal(&mut self, stmt: &Stmt) -> Result<(), String> {
        match stmt {
            Stmt::Block { statements: _ } => self.resolveBlock(stmt)?,
            Stmt::Var {
                name: _,
                initializer: _,
            } => self.resolveVar(stmt)?,
            Stmt::WrafsVar { id, name, source } => {
            	self.resolveLocal(source, *id)?;
            	self.declare(name)?;
            	self.define(name);
            }
			Stmt::HotlinkVar { id, name, source } => {
				self.resolveLocal(source, *id)?;
				self.declare(name)?;
				self.define(name);
			}
			Stmt::GhostVar { name, initializer } => {
				self.declare(name)?;
				self.resolveExpr(initializer)?;
				self.define(name);
			}
            Stmt::Class {
                name,
                methods,
                superclass,
            } => {
                // Resolve superclass, if present
                if let Some(super_expr) = superclass {
                    if let Expr::Variable {
                        id: _,
                        name: super_name,
                    } = super_expr
                    {
                        if super_name.lexeme == name.lexeme {
                            return Err("A class cannot inherit from itself".to_string());
                        }
                    }

                    self.resolveExpr(super_expr)?;
                    self.beginScope();
                    self.scopes
                        .last_mut()
                        .unwrap()
                        .insert("super".to_string(), true);
                }

                // Resolving class
                self.declare(name)?;
                self.define(name);

                // Resolving methods
                self.beginScope();
                self.scopes
                    .last_mut()
                    .unwrap()
                    .insert("this".to_string(), true);
                for method in methods {
                    let declaration = FunctionType::Method;
                    self.resolveFunction(method, declaration)?;
                }
                self.endScope();

                if superclass.is_some() {
                    self.endScope();
                }
            }
            Stmt::Function {
                name: _,
                params: _,
                body: _,
            } => self.resolveFunction(stmt, FunctionType::Function)?,
			Stmt::EntryFunction {
				name: _,
				params: _,
				body: _,
			} => self.resolveEntryFunction(stmt)?,
            Stmt::CmdFunction { name: _, cmd: _ } => self.resolveVar(stmt)?,
            Stmt::Expression { expression } => self.resolveExpr(expression)?,
            Stmt::IfStmt {
                predicate: _,
                then: _,
                els: _,
            } => self.resolveIfStatement(stmt)?,
            Stmt::Print { expression } => self.resolveExpr(expression)?,
            Stmt::ReturnStmt { keyword: _, value } => {
                if self.current_function == FunctionType::None {
                    return Err("Return statement is not allowed outside of a function".to_string());
                }

                if let Some(value) = value {
                    self.resolveExpr(value)?;
                }
            }
            Stmt::WhileStmt { condition, body } => {
                self.resolveExpr(condition)?;
				self.loop_depth += 1;
                let result = self.resolveInternal(body.as_ref());
				self.loop_depth -= 1;
				if let Err(e) = result {
					return Err(e);
				}
            }
			Stmt::ForStmt {
				variable,
				iterable,
				body,
			} => {
				self.beginScope();
				self.declare(variable);
				self.define(variable);
				self.resolveExpr(iterable)?;
				self.loop_depth += 1;
				let result = self.resolveInternal(body.as_ref());
				self.loop_depth -= 1;
				self.endScope();
				if let Err(e) = result {
					return Err(e);
				}
			}
			Stmt::BreakStmt { keyword: _ } => {
				if self.loop_depth == 0 {
					return Err("'break' is not allowed outside of a loop".to_string());
				}
			}
			Stmt::ContinueStmt { keyword: _ } => {
				if self.loop_depth == 0 {
					return Err("'continue' is not allowed outside of a loop".to_string());
				}
			}
        }
        Ok(())
    }

	#[allow(non_snake_case)]
    fn resolveMany(&mut self, stmts: &Vec<&Stmt>) -> Result<(), String> {
        for stmt in stmts {
            self.resolveInternal(stmt)?;
        }

        Ok(())
    }

    pub fn resolve(mut self, stmts: &Vec<&Stmt>) -> Result<HashMap<usize, usize>, String> {
        self.resolveMany(stmts)?;
        Ok(self.locals)
    }

	#[allow(non_snake_case)]
    fn resolveBlock(&mut self, stmt: &Stmt) -> Result<(), String> {
        match stmt {
            Stmt::Block { statements } => {
                self.beginScope();
                self.resolveMany(&statements.iter().map(|b| b.as_ref()).collect())?;
                self.endScope();
            }
            _ => panic!("Wrong type"),
        }

        Ok(())
    }

	#[allow(non_snake_case)]
    fn resolveVar(&mut self, stmt: &Stmt) -> Result<(), String> {
        if let Stmt::Var { name, initializer } = stmt {
            self.declare(name)?;
            self.resolveExpr(initializer)?;
            self.define(name);
        } else if let Stmt::CmdFunction {name, cmd: _} = stmt {
            self.declare(name)?;
            self.define(name);
        } else {
            panic!("Wrong type in resolve var");
        }

        Ok(())
    }

	#[allow(non_snake_case)]
    fn resolveFunction(&mut self, stmt: &Stmt, fn_type: FunctionType) -> Result<(), String> {
        if let Stmt::Function { name, params, body } = stmt {
            self.declare(name)?;
            self.define(name);

            self.resolveFunctionHelper(
                params,
                &body.iter().map(|b| b.as_ref()).collect(),
                fn_type,
            )
        } else {
            panic!("Wrong type in resolve function");
        }
    }

	#[allow(non_snake_case)]
	fn resolveEntryFunction(&mut self, stmt: &Stmt) -> Result<(), String> {
		if let Stmt::EntryFunction { name , params, body} = stmt {
			self.declare(name)?;
			self.define(name);

			self.resolveFunctionHelper(
				params,
				&body.iter().map(|b| b.as_ref()).collect(),
				FunctionType::Function,
			)
		} else {
			panic!("Wrong type in resolve entry function");
		}
	}

	#[allow(non_snake_case)]
    fn resolveIfStatement(&mut self, stmt: &Stmt) -> Result<(), String> {
        if let Stmt::IfStmt {
            predicate,
            then,
            els,
        } = stmt
        {
            self.resolveExpr(predicate)?;
            self.resolveInternal(then.as_ref())?;
            if let Some(els) = els {
                self.resolveInternal(els.as_ref())?;
            }

            Ok(())
        } else {
            panic!("Wrong type in resolve if stmt");
        }
    }

	#[allow(non_snake_case)]
    fn resolveFunctionHelper(
        &mut self,
        params: &Vec<Token>,
        body: &Vec<&Stmt>,
        resolving_function: FunctionType,
    ) -> Result<(), String> {
        let enclosing_function = self.current_function;
        self.current_function = resolving_function;
        self.beginScope();
        for param in params {
            self.declare(param)?;
            self.define(param);
        }
        self.resolveMany(body)?;
        self.endScope();
        self.current_function = enclosing_function;
        Ok(())
    }

	#[allow(non_snake_case)]
    fn beginScope(&mut self) {
        self.scopes.push(HashMap::new());
    }

	#[allow(non_snake_case)]
    fn endScope(&mut self) {
        self.scopes.pop().expect("Stack underflow");
    }

    fn declare(&mut self, name: &Token) -> Result<(), String> {
        let size = self.scopes.len();
        if self.scopes.is_empty() {
            return Ok(());
        }

        if self.scopes[size - 1].contains_key(&name.lexeme.clone()) {
            return Err("A variable with this name is already in scope".to_string());
        }

        self.scopes[size - 1].insert(name.lexeme.clone(), false);

        Ok(())
    }

    fn define(&mut self, name: &Token) {
        if self.scopes.is_empty() {
            return;
        }

        let size = self.scopes.len();
        self.scopes[size - 1].insert(name.lexeme.clone(), true);
    }

    // (i > j) may require different resolution distances
    // { var a = 2; fun fn() { return a;} { var a = 1; var b = fn(); } }
    // (i > 3) -> take id -> store resolution distance
    // (i > 3) ->
    //         -> i -> try to resolve
    //         -> 3 -> try to resolve (trivial)
	#[allow(non_snake_case)]
    fn resolveExpr(&mut self, expr: &Expr) -> Result<(), String> {
        match expr {
            Expr::Variable { id: _, name: _ } => self.resolveExprVar(expr, expr.getId()),
            Expr::Assign {
                id: _,
                name: _,
                value: _,
            } => self.resolveExprAssign(expr, expr.getId()),
            Expr::Binary {
                id: _,
                left,
                operator: _,
                right,
            } => {
                self.resolveExpr(left)?;
                self.resolveExpr(right)
            }
            Expr::Call {
                id: _,
                callee,
                paren: _,
                arguments,
            } => {
                self.resolveExpr(callee.as_ref())?;
                for arg in arguments {
                    self.resolveExpr(arg)?;
                }

                Ok(())
            }
            Expr::Get {
                id: _,
                object,
                name: _,
            } => self.resolveExpr(object),
            Expr::Grouping { id: _, expression } => self.resolveExpr(expression),
            Expr::Literal { id: _, value: _ } => Ok(()),
            Expr::Logical {
                id: _,
                left,
                operator: _,
                right,
            } => {
                self.resolveExpr(left)?;
                self.resolveExpr(right)
            }
            Expr::Set {
                id: _,
                object,
                name: _,
                value,
            } => {
                self.resolveExpr(value)?;
                self.resolveExpr(object)
            }
            Expr::This { id: _, keyword } => {
                if self.current_function != FunctionType::Method {
                    return Err("Cannot use 'this' keyword outside of a class".to_string());
                }
                self.resolveLocal(keyword, expr.getId())
            }
            Expr::Super {
                id: _,
                keyword,
                method: _,
            } => {
                if self.current_function != FunctionType::Method {
                    return Err("Cannot use 'super' keyword outside of a class".to_string());
                }
                if self.scopes.len() < 3 || !self.scopes[self.scopes.len() - 3].contains_key("super") {
                    return Err("Class has no superclass".to_string());
                }
                self.resolveLocal(keyword, expr.getId())
            }
            Expr::Unary {
                id: _,
                operator: _,
                right,
            } => self.resolveExpr(right),
            Expr::AnonFunction {
                id: _,
                paren: _,
                arguments,
                body,
            } => self.resolveFunctionHelper(
                arguments,
                &body.iter().map(|b| b.as_ref()).collect(),
                FunctionType::Function,
            ),
			Expr::ListLiteral { id: _, items } => {
				for item in items {
					self.resolveExpr(item)?;
				}
				Ok(())
			}
			Expr::Index {
				id: _,
				object,
				bracket: _,
				index,
			} => {
				self.resolveExpr(object)?;
				self.resolveExpr(index)
			}
			Expr::IndexSet {
				id: _,
				object,
				bracket: _,
				index,
				value,
			} => {
				self.resolveExpr(value)?;
				self.resolveExpr(object)?;
				self.resolveExpr(index)
			}
        }
    }

	#[allow(non_snake_case)]
    fn resolveExprVar(&mut self, expr: &Expr, resolve_id: usize) -> Result<(), String> {
        match expr {
            Expr::Variable { id: _, name } => {
                if !self.scopes.is_empty() {
                    if let Some(false) = self.scopes[self.scopes.len() - 1].get(&name.lexeme) {
                        return Err("Can't read local variable in its own initializer".to_string());
                    }
                }

                self.resolveLocal(name, resolve_id)
            }
            Expr::Call {
                id: _,
                callee,
                paren: _,
                arguments: _,
            } => match callee.as_ref() {
                Expr::Variable { id: _, name } => self.resolveLocal(&name, resolve_id),
                _ => panic!("Wrong type in resolveExprVar"),
            },
            _ => panic!("Wrong type in resolveExprVar"),
        }
    }

	#[allow(non_snake_case)]
    fn resolveLocal(&mut self, name: &Token, resolve_id: usize) -> Result<(), String> {
        let size = self.scopes.len();
        if size == 0 {
            return Ok(());
        }

        for i in (0..=(size - 1)).rev() {
            let scope = &self.scopes[i];
            if scope.contains_key(&name.lexeme) {
                self.locals.insert(resolve_id, size - 1 - i);
                return Ok(());
            }
        }

        // Assume it's global
        Ok(())
    }

	#[allow(non_snake_case)]
    fn resolveExprAssign(&mut self, expr: &Expr, resolve_id: usize) -> Result<(), String> {
        if let Expr::Assign { id: _, name, value } = expr {
            self.resolveExpr(value.as_ref())?;
            self.resolveLocal(name, resolve_id)?;
        } else {
            panic!("Wrong type in resolve assign");
        }

        Ok(())
    }
}