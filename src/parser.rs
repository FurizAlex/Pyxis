use crate::expr::{Expr, Expr::*, LiteralValue};
use crate::scanner::{Token, TokenType, TokenType::*};
use crate::stmt::Stmt;

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
    next_id: usize,
}

#[derive(Debug)]
enum FunctionKind {
    Function,
    Method,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            current: 0,
            next_id: 0,
        }
    }

	#[allow(non_snake_case)]
    fn getId(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;

        id
    }

    pub fn parse(&mut self) -> Result<Vec<Stmt>, String> {
        let mut stmts = vec![];
        let mut errs = vec![];

		while !self.isAtEnd() {		
		    while self.matchToken(Newline) {}
		    if self.isAtEnd() {
		        break;
		    }
		    let stmt = self.declaration();

		    match stmt {
		        Ok(s) => stmts.push(s),
		        Err(msg) => {
		            errs.push(msg);
		            self.synchronize();
		        }
		    }
		}

        if errs.len() == 0 {
            Ok(stmts)
        } else {
            Err(errs.join("\n"))
        }
    }

    fn declaration(&mut self) -> Result<Stmt, String> {
        if self.matchToken(Var) {
            self.varDeclaration()
        } else if self.matchToken(Fun) {
            self.function(FunctionKind::Function)
		} else if self.matchToken(Defi) {
			self.entryFunction()
        } else if self.matchToken(Class) {
            self.classDeclaration()
        } else if self.matchToken(At) {
        	self.decorator()
        } else {
            self.statement()
        }
    }

	#[allow(non_snake_case)]
	fn classDeclaration(&mut self) -> Result<Stmt, String> {
	    let name = self.consume(Identifier, "Expected name after 'class' keyword.")?;
	
		if self.matchToken(LeftParen) {
			self.consume(RightParen, "Expected ')' after class name (no parameters are supported here yet)")?;
		}
	    let superclass = if self.matchToken(TokenType::Less) {
	        self.consume(Identifier, "Expected superclass name after '<'.")?;
		
	        Some(Expr::Variable {
	            id: self.getId(),
	            name: self.previous(),
	        })
	    } else {
	        None
	    };
	
	    self.consume(Colon, "Expected ':' after class declaration")?;
	    self.consume(Newline, "Expected newline")?;
	
	    let mut methods = vec![];
	
	    if self.matchToken(Indent) {
	        while !self.check(Dedent) && !self.isAtEnd() {
				self.consume(Fun, "Expected ' func' before method name")?;
	            let method = self.function(FunctionKind::Method)?;
	            methods.push(Box::new(method));
	        }
		
	        self.consume(Dedent, "Expected end of class block")?;
	    }
	
	    Ok(Stmt::Class {
	        name,
	        methods,
	        superclass,
	    })
	}
	
	#[allow(non_snake_case)]
	fn wrafsDeclaration(&mut self) -> Result<Stmt, String> {
		self.consume(Var, "Expected 'var' after '@wrafs'")?;
		let name = self.consume(Identifier, "Expected variable name after 'var'")?;
	
		self.consume(Equal, "Expected '::' after variable name")?;
		let source = self.consume(
			Identifier,
			"Expected an existing variable name after '::' in a @wrafs declaration (expressions are not allowed here <-> @wrafs can only link with an another variable directly)"
		)?;
		self.consume(Bang, "Expected '[!]' after @wrafs declaration")?;
		Ok(Stmt::WrafsVar {id: self.getId(), name, source})
	}

	#[allow(non_snake_case)]
	fn hotlinkDeclaration(&mut self) -> Result<Stmt, String> {
		self.consume(Var, "Expected 'var' after '@hotlink'")?;
		let name = self.consume(Identifier, "Expected variable named after var")?;
		self.consume(Equal, "Expected '::' after variable name")?;
		let source = self.consume(Identifier, "Expected an existing variable name after '::' in a @hotlink declaration (expressions are not allowed here -- @hotlink can only link to another variable directly")?;
		self.consume(Bang, "Expected '[!]' after @hotlink declaration")?;
		Ok(Stmt::HotlinkVar { id: self.getId(), name, source, })
	}

	#[allow(non_snake_case)]
	fn ghostDeclaration(&mut self) -> Result<Stmt, String> {
		self.consume(Var, "Expected 'var' after @ghost")?;
		match self.varDeclaration()? {
			Stmt::Var { name, initializer } => Ok(Stmt::GhostVar { name, initializer }),
			_ => panic!("varDeclaration() returned something other than Stmt::Var"),
		}
	}

    fn function(&mut self, kind: FunctionKind) -> Result<Stmt, String> {
        let name = self.consume(Identifier, &format!("Expected {kind:?} name"))?;

        if self.matchToken(Gets) {
            let cmd_body = self.consume(StringLit, "Expected command body")?; 
            self.consume(Bang, "Expected '[!]' after command body")?;

            return Ok(Stmt::CmdFunction {
                name,
                cmd: cmd_body.lexeme,
            });
        }

        self.consume(LeftParen, &format!("Expected '(' after {kind:?} name"))?;

        let mut parameters = vec![];
        if !self.check(RightParen) {
            loop {
                if parameters.len() >= 255 {
                    let location = self.peek().line_number;
                    return Err(format!(
                        "Line {location}: Cant have more than 255 arguments"
                    ));
                }

                let param = self.consume(Identifier, "Expected parameter name")?;
                parameters.push(param);

                if !self.matchToken(Comma) {
                    break;
                }
            }
        }
        self.consume(RightParen, "Expected ')' after parameters.")?;

        self.consume(Colon, "Expected ':' before block")?;
		self.consume(Newline, "Expected newline after ':'")?;
		self.consume(Indent, "Expected indented block")?;
        let body = match self.blockStatement()? {
            Stmt::Block { statements } => statements,
            _ => panic!("Block statement parsed something that was not a block"),
        };

        Ok(Stmt::Function {
            name,
            params: parameters,
            body,
        })
    }

	#[allow(non_snake_case)]
	fn entryFunction(&mut self) -> Result<Stmt, String> {
		let name = self.consume(Identifier, "Expected name after 'defi' keyword")?;

		self.consume(LeftParen, "Expected '(' after function name")?;

		let mut parameters = vec![];
		if !self.check(RightParen) {
			loop {
				if parameters.len() >= 255 {
					let location = self.peek().line_number;
					return Err(format!(
						"Line {location}: Can't have more than 255 arguments"
					));
				}
				let param = self.consume(Identifier, "Expected parameter name")?;

				parameters.push(param);
				if !self.matchToken(Comma) {
					break;
				}
			}
		}
		self.consume(RightParen, "Expected ')' after paremeters.")?;
		self.consume(Colon, "Expected ':' before block")?;
		self.consume(Newline, "Expected newline after ':'")?;
		self.consume(Indent, "Expected indented block")?;

		let body = match self.blockStatement()? {
			Stmt::Block { statements } => statements,
			_ => panic!("Block statement parsed something that was not a block"),
		};
		Ok(Stmt::EntryFunction {
			name,
			params: parameters,
			body,
		})
	}

	#[allow(non_snake_case)]
    fn varDeclaration(&mut self) -> Result<Stmt, String> {
        let token = self.consume(Identifier, "Expected variable name")?;

        let initializer;
        if self.matchToken(Equal) {
            initializer = self.expression()?;
        } else {
            initializer = Literal {
                id: self.getId(),
                value: LiteralValue::Nil,
            };
        }

        self.consume(Bang, "Expected '[!]' after variable declaration")?;

        Ok(Stmt::Var {
            name: token,
            initializer,
        })
    }

    fn statement(&mut self) -> Result<Stmt, String> {
        if self.matchToken(Print) {
            self.printStatement()
        } else if self.matchToken(If) {
            self.ifStatement()
        } else if self.matchToken(While) {
            self.whileStatement()
        } else if self.matchToken(For) {
            self.forStatement()
        } else if self.matchToken(Return) {
            self.returnStatement()
		} else if self.matchToken(Break) {
			self.breakStatement()
		} else if self.matchToken(Continue) {
			self.continueStatement()
        } else {
            self.expressionStatement()
        }
    }

	#[allow(non_snake_case)]
    fn returnStatement(&mut self) -> Result<Stmt, String> {
        let keyword = self.previous();
        let value;
        if !self.check(Bang) {
            // NOT return;
            value = Some(self.expression()?);
        } else {
            value = None;
        }
        self.consume(Bang, "Expected '[!]' after return value;")?;

        Ok(Stmt::ReturnStmt { keyword, value })
    }

	#[allow(non_snake_case)]
	fn forStatement(&mut self) -> Result<Stmt, String> {
	    let name = self.consume(Identifier, "Expected loop variable.")?;

	    self.consume(In, "Expected 'in' after loop variable.")?;

	    let iterable = self.expression()?;

	    self.consume(Colon, "Expected ':' after iterable.")?;
	    self.consume(Newline, "Expected newline after ':'")?;
	    self.consume(Indent, "Expected indented block")?;

	    let body = self.blockStatement()?;

	    Ok(Stmt::ForStmt {
	        variable: name,
	        iterable,
	        body: Box::new(body),
	    })
	}

	#[allow(non_snake_case)]
	fn whileStatement(&mut self) -> Result<Stmt, String> {
	    let condition = self.expression()?;

	    self.consume(Colon, "Expected ':' after while condition")?;
	    self.consume(Newline, "Expected newline after ':'")?;
	    self.consume(Indent, "Expected indented block")?;

	    let body = self.blockStatement()?;

	    Ok(Stmt::WhileStmt {
	        condition,
	        body: Box::new(body),
	    })
	}

	#[allow(non_snake_case)]
	fn ifStatement(&mut self) -> Result<Stmt, String> {
	    let predicate = self.expression()?;

	    self.consume(Colon, "Expected ':' after if condition")?;
	    self.consume(Newline, "Expected newline after ':'")?;
	    self.consume(Indent, "Expected indented block")?;

	    let then = Box::new(self.blockStatement()?);

	    let els = if self.matchToken(Unif) {
			Some(Box::new(self.ifStatement()?))
		} else if self.matchToken(Else) {
	        self.consume(Colon, "Expected ':' after else")?;
	        self.consume(Newline, "Expected newline after ':'")?;
	        self.consume(Indent, "Expected indented block")?;

	        Some(Box::new(self.blockStatement()?))
	    } else {
	        None
	    };

	    Ok(Stmt::IfStmt {
	        predicate,
	        then,
	        els,
	    })
	}

	#[allow(non_snake_case)]
    fn blockStatement(&mut self) -> Result<Stmt, String> {
        let mut statements = vec![];

        while !self.check(Dedent) && !self.isAtEnd() {

		    while self.matchToken(Newline) {}

		    if self.check(Dedent) {
		        break;
		    }
		
		    let decl = self.declaration()?;
		    statements.push(Box::new(decl));
		}

        self.consume(Dedent, "Expected end of block")?;
        Ok(Stmt::Block { statements })
    }

	#[allow(non_snake_case)]
    fn printStatement(&mut self) -> Result<Stmt, String> {
        let value = self.expression()?;
        self.consume(Bang, "Expected '[!]' after value.")?;
        Ok(Stmt::Print { expression: value })
    }

	#[allow(non_snake_case)]
    fn expressionStatement(&mut self) -> Result<Stmt, String> {
        let expr = self.expression()?;
        self.consume(Bang, "Expected '[!]' after expression.")?;
        Ok(Stmt::Expression { expression: expr })
    }

	#[allow(non_snake_case)]
	fn breakStatement(&mut self) -> Result<Stmt, String> {
		let keyword = self.previous();
		self.consume(Bang, "Expected '[!]' after 'break'")?;
		Ok(Stmt::BreakStmt { keyword })
	}

	#[allow(non_snake_case)]
	fn continueStatement(&mut self) -> Result<Stmt, String> {
		let keyword = self.previous();
		self.consume(Bang, "Expected '[!]' after 'continue'")?;
		Ok(Stmt::ContinueStmt { keyword })
	}
	
	#[allow(non_snake_case)]
	fn decorator(&mut self) -> Result<Stmt, String> {
		let tag = self.consume(Identifier, "Expected decorator name after @")?;
		
		match tag.lexeme.as_str() {
			"wrafs" => self.wrafsDeclaration(),
			"hotlink" => self.hotlinkDeclaration(),
			"ghost" => self.ghostDeclaration(),
			"export" => Err(format!(
				"Line {}: 'export' is recognized but not implemented yet (requires a module system, which doesn't exist yet", tag.line_number
			)),
			"bind" => Err(format!(
				"Line {}: '@bind' is recognized but not implemented yet (requires an event system, which doesn't exist yet", tag.line_number
			)),
			other => Err(format!(
				"Line {}: '@{}'' is not a recognized decorator",
				tag.line_number, other
			)),
		}
	}

    fn expression(&mut self) -> Result<Expr, String> {
        self.assignment()
    }

	#[allow(non_snake_case)]
    fn functionExpression(&mut self) -> Result<Expr, String> {
        let paren = self.consume(LeftParen, "Expected '(' after anonymous function")?;
        let mut parameters = vec![];
        if !self.check(RightParen) {
            loop {
                if parameters.len() >= 255 {
                    let location = self.peek().line_number;
                    return Err(format!(
                        "Line {location}: Cant have more than 255 arguments"
                    ));
                }

                let param = self.consume(Identifier, "Expected parameter name")?;
                parameters.push(param);

                if !self.matchToken(Comma) {
                    break;
                }
            }
        }
        self.consume(
            RightParen,
            "Expected ')' after anonymous function parameters",
        )?;

        self.consume(Colon, "Expected ':' after anonymous function declaration",)?;
		self.consume(Newline, "Expected newline after ':'",)?;
		self.consume(Indent, "Expected indented block",)?;

        let body = match self.blockStatement()? {
            Stmt::Block { statements } => statements,
            _ => panic!("Block statement parsed something that was not a block"),
        };

        Ok(Expr::AnonFunction {
            id: self.getId(),
            paren,
            arguments: parameters,
            body,
        })
    }

    fn assignment(&mut self) -> Result<Expr, String> {
        // a = 2; NOT var a = 2;
        let expr = self.pipe()?; // a |> f = 2;

        if self.matchToken(Equal) {
            let value = self.expression()?;

            match expr {
                Variable { id: _, name } => Ok(Assign {
                    id: self.getId(),
                    name,
                    value: Box::from(value),
                }),
                Get {
                    id: _,
                    object,
                    name,
                } => Ok(Set {
                    id: self.getId(),
                    object,
                    name,
                    value: Box::new(value),
                }),
				Expr::Index {
					id: _,
					object,
					bracket,
					index,
				} => Ok(Expr::IndexSet {
					id: self.getId(),
					object,
					bracket,
					index,
					value: Box::new(value),
				}),
                _ => Err("Invalid assignment target.".to_string()),
            }
        } else {
            Ok(expr)
        }
    }

    fn pipe(&mut self) -> Result<Expr, String> {
        // expr |> f
        // expr |> f1 |> f2
        // expr |> (f1 |> f2)
        // expr |> (f1 |> (f2 |> f3))
        // (expr |> f1) |> f2

        // expr |> fun (a) { return a + 1; }
        // expr |> a -> a + 1
        let mut expr = self.or()?;
        while self.matchToken(Pipe) {
            let pipe = self.previous();
            let function = self.or()?;

            expr = Call {
                id: self.getId(),
                callee: Box::new(function),
                paren: pipe,
                arguments: vec![expr],
            };
        }
        Ok(expr)
    }

    fn or(&mut self) -> Result<Expr, String> {
        let mut expr = self.and()?;

        while self.matchToken(Or) {
            let operator = self.previous();
            let right = self.and()?;

            expr = Logical {
                id: self.getId(),
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn and(&mut self) -> Result<Expr, String> {
        let mut expr = self.equality()?;

        while self.matchToken(And) {
            let operator = self.previous();
            let right = self.equality()?;
            expr = Logical {
                id: self.getId(),
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn equality(&mut self) -> Result<Expr, String> {
        let mut expr = self.comparison()?;
        while self.matchTokens(&[BangEqual, EqualEqual]) {
            let operator = self.previous();
            let rhs = self.comparison()?;
            expr = Binary {
                id: self.getId(),
                left: Box::from(expr),
                operator,
                right: Box::from(rhs),
            };
        }

        Ok(expr)
    }

    fn comparison(&mut self) -> Result<Expr, String> {
        let mut expr = self.term()?;

        while self.matchTokens(&[Greater, GreaterEqual, Less, LessEqual]) {
            let op = self.previous();
            let rhs = self.term()?;
            expr = Binary {
                id: self.getId(),
                left: Box::from(expr),
                operator: op,
                right: Box::from(rhs),
            };
        }

        Ok(expr)
    }

    fn term(&mut self) -> Result<Expr, String> {
        let mut expr = self.factor()?;

        while self.matchTokens(&[Minus, Plus]) {
            let op = self.previous();
            let rhs = self.factor()?;
            expr = Binary {
                id: self.getId(),
                left: Box::from(expr),
                operator: op,
                right: Box::from(rhs),
            };
        }

        Ok(expr)
    }

    fn factor(&mut self) -> Result<Expr, String> {
        let mut expr = self.unary()?;
        while self.matchTokens(&[Slash, Star, Percent]) {
            let op = self.previous();
            let rhs = self.unary()?;
            expr = Binary {
                id: self.getId(),
                left: Box::from(expr),
                operator: op,
                right: Box::from(rhs),
            };
        }

        Ok(expr)
    }

    fn unary(&mut self) -> Result<Expr, String> {
        if self.matchTokens(&[Bang, Minus]) {
            let op = self.previous();
            let rhs = self.unary()?;
            Ok(Unary {
                id: self.getId(),
                operator: op,
                right: Box::from(rhs),
            })
        } else {
            self.call()
        }
    }

    fn call(&mut self) -> Result<Expr, String> {
        let mut expr = self.primary()?;

        loop {
            if self.matchToken(LeftParen) {
                expr = self.finishCall(expr)?;
            } else if self.matchToken(Dot) {
                let name = self.consume(Identifier, "Expected token after dot-accessor")?;
                expr = Get {
                    id: self.getId(),
                    object: Box::new(expr),
                    name,
                };
            } else if self.matchToken(LeftBracket) {
				let bracket = self.previous();
				let index = self.expression()?;

				self.consume(RightBracket, "Expected ']' after index")?;
				expr = Expr::Index {
					id: self.getId(),
					object: Box::new(expr),
					bracket,
					index: Box::new(index),
				};
			} else {
                break;
            }
        }

        Ok(expr)
    }

	#[allow(non_snake_case)]
    fn finishCall(&mut self, callee: Expr) -> Result<Expr, String> {
        let mut arguments = vec![];

        if !self.check(RightParen) {
            loop {
                let arg = self.expression()?;
                arguments.push(arg);
                if arguments.len() >= 255 {
                    let location = self.peek().line_number;
                    return Err(format!(
                        "Line {location}: Cant have more than 255 arguments"
                    ));
                }

                if !self.matchToken(Comma) {
                    break;
                }
            }
        }
        let paren = self.consume(RightParen, "Expected ')' after arguments.")?;

        Ok(Call {
            id: self.getId(),
            callee: Box::new(callee),
            paren,
            arguments,
        })
    }

    fn primary(&mut self) -> Result<Expr, String> {
        let token = self.peek();
        let result;
        match token.token_type {
            LeftParen => {
                self.advance();
                let expr = self.expression()?;
                self.consume(RightParen, "Expected ')'")?;
                result = Grouping {
                    id: self.getId(),
                    expression: Box::from(expr),
                };
            }
			LeftBracket => {
				self.advance();
				let mut items = vec![];

				if !self.check(RightBracket) {
					loop {
						let item = self.expression()?;
						items.push(item);

						if !self.matchToken(Comma) {
							break;
						}
					}
				}
				self.consume(RightBracket, "Expected ']' after array")?;
				result = Expr::ListLiteral { id: self.getId(), items, };
			}
            False | True | Nil | Number | StringLit => {
                self.advance();
                result = Literal {
                    id: self.getId(),
                    value: LiteralValue::from_token(token),
                }
            }
            Identifier => {
                self.advance();
                result = Variable {
                    id: self.getId(),
                    name: self.previous(),
                };
            }
            TokenType::This => {
                self.advance();
                result = Expr::This {
                    id: self.getId(),
                    keyword: token,
                };
            }
            TokenType::Super => {
                // Should always occur with a method call
                self.advance();
                self.consume(TokenType::Dot, "Expected '.' after 'super'.")?;
                let method =
                    self.consume(TokenType::Identifier, "Expected superclass method name.")?;
                result = Expr::Super {
                    id: self.getId(),
                    keyword: token,
                    method,
                };
            }
            Fun => {
                self.advance();
                result = self.functionExpression()?;
            }
            _ => return Err("Expected expression".to_string()),
        }

        Ok(result)
    }

    fn consume(&mut self, token_type: TokenType, msg: &str) -> Result<Token, String> {
        let token = self.peek();
        if token.token_type == token_type {
            self.advance();
            let token = self.previous();
            Ok(token)
        } else {
            Err(format!("Line {}: {}", token.line_number, msg))
        }
    }

    fn check(&mut self, typ: TokenType) -> bool {
        self.peek().token_type == typ
    }

	#[allow(non_snake_case)]
    fn matchToken(&mut self, typ: TokenType) -> bool {
        if self.isAtEnd() {
            false
        } else {
            if self.peek().token_type == typ {
                self.advance();
                true
            } else {
                false
            }
        }
    }

	#[allow(non_snake_case)]
    fn matchTokens(&mut self, typs: &[TokenType]) -> bool {
        for typ in typs {
            if self.matchToken(*typ) {
                return true;
            }
        }

        false
    }

    fn advance(&mut self) -> Token {
        if !self.isAtEnd() {
            self.current += 1;
        }

        self.previous()
    }

    fn peek(&mut self) -> Token {
        self.tokens[self.current].clone()
    }

    fn previous(&mut self) -> Token {
        self.tokens[self.current - 1].clone()
    }

	#[allow(non_snake_case)]
    fn isAtEnd(&mut self) -> bool {
        self.peek().token_type == Eof
    }

    fn synchronize(&mut self) {
        self.advance();

        while !self.isAtEnd() {
            if self.previous().token_type == Bang {
                return;
            }

            match self.peek().token_type {
                Class | Fun | Var | For | If | While | Print | Return => return,
                _ => (),
            }

            self.advance();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::{LiteralValue::*, Scanner};

    #[test]
    fn test_addition() {
        let one = Token {
            token_type: Number,
            lexeme: "1".to_string(),
            literal: Some(FValue(1.0)),
            line_number: 0,
        };
        let plus = Token {
            token_type: Plus,
            lexeme: "+".to_string(),
            literal: None,
            line_number: 0,
        };
        let two = Token {
            token_type: Number,
            lexeme: "2".to_string(),
            literal: Some(FValue(2.0)),
            line_number: 0,
        };
        let semicol = Token {
            token_type: Bang,
            lexeme: ";".to_string(),
            literal: None,
            line_number: 0,
        };
        let eof = Token {
            token_type: Eof,
            lexeme: "".to_string(),
            literal: None,
            line_number: 0,
        };

        let tokens = vec![one, plus, two, semicol, eof];
        let mut parser = Parser::new(tokens);

        let parsed_expr = parser.parse().unwrap();
        let string_expr = parsed_expr[0].to_string();

        assert_eq!(string_expr, "(+ 1 2)");
    }

    #[test]
    fn test_comparison() {
        let source = "1 + 2 == 5 + 7;";
        let mut scanner = Scanner::new(source);
        let tokens = scanner.scan_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let parsed_expr = parser.parse().unwrap();
        let string_expr = parsed_expr[0].to_string();

        assert_eq!(string_expr, "(== (+ 1 2) (+ 5 7))");
    }

    #[test]
    fn test_eq_with_paren() {
        let source = "1 == (2 + 2);";
        let mut scanner = Scanner::new(source);
        let tokens = scanner.scan_tokens().unwrap();
        let mut parser = Parser::new(tokens);
        let parsed_expr = parser.parse().unwrap();
        let string_expr = parsed_expr[0].to_string();

        assert_eq!(string_expr, "(== 1 (group (+ 2 2)))");
    }
}