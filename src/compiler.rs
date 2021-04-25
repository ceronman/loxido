use crate::{
    allocator::{Allocator, Reference},
    chunk::{Instruction, Value},
    error::LoxError,
    function::Upvalue,
    function::{FunctionType, LoxFunction},
    scanner::{Scanner, Token, TokenType},
};
use std::collections::HashMap;
use std::{convert::TryFrom, mem};

#[derive(Copy, Clone, PartialOrd, PartialEq)]
enum Precedence {
    None,
    Assignment, // =
    Or,         // or
    And,        // and
    Equality,   // == !=
    Comparison, // < > <= >=
    Term,       // + -
    Factor,     // * /
    Unary,      // ! -
    Call,       // . ()
    Primary,
}

impl Precedence {
    fn next(&self) -> Precedence {
        match self {
            Precedence::None => Precedence::Assignment,
            Precedence::Assignment => Precedence::Or,
            Precedence::Or => Precedence::And,
            Precedence::And => Precedence::Equality,
            Precedence::Equality => Precedence::Comparison,
            Precedence::Comparison => Precedence::Term,
            Precedence::Term => Precedence::Factor,
            Precedence::Factor => Precedence::Unary,
            Precedence::Unary => Precedence::Call,
            Precedence::Call => Precedence::Primary,
            Precedence::Primary => Precedence::None,
        }
    }
}

type ParseFn<'sourcecode> = fn(&mut Parser<'sourcecode>, can_assing: bool) -> ();

#[derive(Copy, Clone)]
struct ParseRule<'sourcecode> {
    prefix: Option<ParseFn<'sourcecode>>,
    infix: Option<ParseFn<'sourcecode>>,
    precedence: Precedence,
}

impl<'sourcecode> ParseRule<'sourcecode> {
    fn new(
        prefix: Option<ParseFn<'sourcecode>>,
        infix: Option<ParseFn<'sourcecode>>,
        precedence: Precedence,
    ) -> ParseRule<'sourcecode> {
        ParseRule {
            prefix,
            infix,
            precedence,
        }
    }
}

#[derive(Copy, Clone)]
struct Local<'sourcecode> {
    name: Token<'sourcecode>,
    depth: i32,
    is_captured: bool,
}

impl<'sourcecode> Local<'sourcecode> {
    fn new(name: Token<'sourcecode>, depth: i32) -> Self {
        Local {
            name,
            depth,
            is_captured: false,
        }
    }
}

struct Compiler<'sourcecode> {
    enclosing: Option<Box<Compiler<'sourcecode>>>,
    function: LoxFunction,
    function_type: FunctionType,
    locals: Vec<Local<'sourcecode>>,
    scope_depth: i32,
}

impl<'sourcecode> Compiler<'sourcecode> {
    const LOCAL_COUNT: usize = std::u8::MAX as usize + 1;

    fn new(enclosing: Option<Box<Compiler<'sourcecode>>>, kind: FunctionType) -> Box<Self> {
        let mut compiler = Compiler {
            enclosing,
            function: LoxFunction::default(),
            function_type: kind,
            locals: Vec::with_capacity(Compiler::LOCAL_COUNT),
            scope_depth: 0,
        };

        let token = match kind {
            FunctionType::Method | FunctionType::Initializer => Token::synthetic("this"),
            _ => Token::synthetic(""),
        };
        compiler.locals.push(Local::new(token, 0));
        Box::new(compiler)
    }

    fn resolve_local(&mut self, name: Token, errors: &mut Vec<&'static str>) -> Option<u8> {
        for (i, local) in self.locals.iter().enumerate().rev() {
            if name.lexeme == local.name.lexeme {
                if local.depth == -1 {
                    errors.push("Can't read local variable in its own initializer.");
                }
                return Some(i as u8);
            }
        }
        None
    }

    fn resolve_upvalue(&mut self, name: Token, errors: &mut Vec<&'static str>) -> Option<u8> {
        if let Some(enclosing) = self.enclosing.as_mut() {
            if let Some(index) = enclosing.resolve_local(name, errors) {
                enclosing.locals[index as usize].is_captured = true;
                return Some(self.add_upvalue(index, true, errors));
            }
            if let Some(index) = enclosing.resolve_upvalue(name, errors) {
                return Some(self.add_upvalue(index, false, errors));
            }
        }
        None
    }

    fn add_upvalue(&mut self, index: u8, is_local: bool, errors: &mut Vec<&'static str>) -> u8 {
        for (i, upvalue) in self.function.upvalues.iter().enumerate() {
            if upvalue.index == index && upvalue.is_local == is_local {
                return i as u8;
            }
        }
        let count = self.function.upvalues.len();

        if count == Compiler::LOCAL_COUNT {
            errors.push("Too many closure variables in function.");
            return 0;
        }

        let upvalue = Upvalue { index, is_local };
        self.function.upvalues.push(upvalue);
        count as u8
    }

    fn is_local_declared(&self, name: Token) -> bool {
        for local in self.locals.iter().rev() {
            if local.depth != -1 && local.depth < self.scope_depth {
                return false;
            }
            if local.name.lexeme == name.lexeme {
                return true;
            }
        }
        false
    }
}

struct ClassCompiler {
    enclosing: Option<Box<ClassCompiler>>,
    has_superclass: bool,
}

impl ClassCompiler {
    fn new(enclosing: Option<Box<ClassCompiler>>) -> Box<Self> {
        Box::new(ClassCompiler {
            enclosing,
            has_superclass: false,
        })
    }
}

struct Parser<'sourcecode> {
    scanner: Scanner<'sourcecode>,
    compiler: Box<Compiler<'sourcecode>>,
    class_compiler: Option<Box<ClassCompiler>>,
    allocator: &'sourcecode mut Allocator,
    current: Token<'sourcecode>,
    previous: Token<'sourcecode>,
    had_error: bool,
    panic_mode: bool,
    resolver_errors: Vec<&'static str>,
    rules: HashMap<TokenType, ParseRule<'sourcecode>>,
}

impl<'sourcecode> Parser<'sourcecode> {
    fn new(code: &'sourcecode str, allocator: &'sourcecode mut Allocator) -> Parser<'sourcecode> {
        let mut rules = HashMap::new();

        let mut rule = |kind, prefix, infix, precedence| {
            rules.insert(kind, ParseRule::new(prefix, infix, precedence));
        };

        use Precedence as P;
        use TokenType::*;
        rule(
            LeftParen,
            Some(Parser::grouping),
            Some(Parser::call),
            P::Call,
        );
        rule(RightParen, None, None, P::None);
        rule(LeftBrace, None, None, P::None);
        rule(RightBrace, None, None, P::None);
        rule(Comma, None, None, P::None);
        rule(Dot, None, Some(Parser::dot), P::Call);
        rule(Minus, Some(Parser::unary), Some(Parser::binary), P::Term);
        rule(Plus, None, Some(Parser::binary), P::Term);
        rule(Semicolon, None, None, P::None);
        rule(Slash, None, Some(Parser::binary), P::Factor);
        rule(Star, None, Some(Parser::binary), P::Factor);
        rule(Bang, Some(Parser::unary), None, P::None);
        rule(BangEqual, None, Some(Parser::binary), P::Equality);
        rule(Equal, None, None, P::None);
        rule(EqualEqual, None, Some(Parser::binary), P::Equality);
        rule(Greater, None, Some(Parser::binary), P::Comparison);
        rule(GreaterEqual, None, Some(Parser::binary), P::Comparison);
        rule(Less, None, Some(Parser::binary), P::Comparison);
        rule(LessEqual, None, Some(Parser::binary), P::Comparison);
        rule(Identifier, Some(Parser::variable), None, P::None);
        rule(String, Some(Parser::string), None, P::None);
        rule(Number, Some(Parser::number), None, P::None);
        rule(And, None, Some(Parser::and_op), P::And);
        rule(Class, None, None, P::None);
        rule(Else, None, None, P::None);
        rule(False, Some(Parser::literal), None, P::None);
        rule(For, None, None, P::None);
        rule(Fun, None, None, P::None);
        rule(If, None, None, P::None);
        rule(Nil, Some(Parser::literal), None, P::None);
        rule(Or, None, Some(Parser::or_op), P::Or);
        rule(Print, None, None, P::None);
        rule(Return, None, None, P::None);
        rule(Super, Some(Parser::super_), None, P::None);
        rule(This, Some(Parser::this), None, P::None);
        rule(True, Some(Parser::literal), None, P::None);
        rule(Var, None, None, P::None);
        rule(While, None, None, P::None);
        rule(Error, None, None, P::None);
        rule(Eof, None, None, P::None);

        Parser {
            scanner: Scanner::new(code),
            compiler: Compiler::new(None, FunctionType::Script),
            class_compiler: None,
            allocator,
            current: Token::synthetic(""),
            previous: Token::synthetic(""),
            had_error: false,
            panic_mode: false,
            resolver_errors: Vec::new(),
            rules,
        }
    }

    fn compile(mut self) -> Result<Reference<LoxFunction>, LoxError> {
        self.advance();

        while !self.matches(TokenType::Eof) {
            self.declaration();
        }

        self.emit_return();

        #[cfg(feature = "debug_trace_execution")]
        if !self.had_error {
            self.compiler.function.chunk.disassemble("code");
        }

        if self.had_error {
            Err(LoxError::CompileError)
        } else {
            Ok(self.allocator.alloc(self.compiler.function))
        }
    }

    fn expression(&mut self) {
        self.parse_precedence(Precedence::Assignment);
    }

    fn expression_statement(&mut self) {
        self.expression();
        self.consume(TokenType::Semicolon, "Expect ';' after expression.");
        self.emit(Instruction::Pop);
    }

    fn declaration(&mut self) {
        if self.matches(TokenType::Class) {
            self.class_declaration();
        } else if self.matches(TokenType::Fun) {
            self.fun_declaration();
        } else if self.matches(TokenType::Var) {
            self.var_declaration();
        } else {
            self.statement();
        }

        if self.panic_mode {
            self.synchronize();
        }
    }

    fn class_declaration(&mut self) {
        self.consume(TokenType::Identifier, "Expect class name.");
        let class_name = self.previous;
        let name_constant = self.identifier_constant(class_name);
        self.declare_variable();
        self.emit(Instruction::Class(name_constant));
        self.define_variable(name_constant);

        let old_class_compiler = self.class_compiler.take();
        let new_class_compiler = ClassCompiler::new(old_class_compiler);
        self.class_compiler.replace(new_class_compiler);

        if self.matches(TokenType::Less) {
            self.consume(TokenType::Identifier, "Expect superclass name.");
            self.variable(false);
            if class_name.lexeme == self.previous.lexeme {
                self.error("A class can't inherit from itself.");
            }
            self.begin_scope();
            self.add_local(Token::synthetic("super"));
            self.define_variable(0);
            self.named_variable(class_name, false);
            self.emit(Instruction::Inherit);
            self.class_compiler.as_mut().unwrap().has_superclass = true;
        }

        self.named_variable(class_name, false);
        self.consume(TokenType::LeftBrace, "Expect '{' before class body.");
        while !self.check(TokenType::RightBrace) && !self.check(TokenType::Eof) {
            self.method();
        }
        self.consume(TokenType::RightBrace, "Expect '}' after class body.");
        self.emit(Instruction::Pop);
        if self.class_compiler.as_ref().unwrap().has_superclass {
            self.end_scope();
        }

        match self.class_compiler.take() {
            Some(c) => self.class_compiler = c.enclosing,
            None => self.class_compiler = None,
        }
    }

    fn fun_declaration(&mut self) {
        let global = self.parse_variable("Expect function name.");
        self.mark_initialized();
        self.function(FunctionType::Function);
        self.define_variable(global);
    }

    fn push_compiler(&mut self, kind: FunctionType) {
        let new_compiler = Compiler::new(None, kind);
        let old_compiler = mem::replace(&mut self.compiler, new_compiler);
        self.compiler.enclosing = Some(old_compiler);
        let function_name = self.allocator.intern(self.previous.lexeme.to_owned());
        self.compiler.function.name = function_name;
    }

    fn pop_compiler(&mut self) -> LoxFunction {
        self.emit_return();
        match self.compiler.enclosing.take() {
            Some(enclosing) => {
                let compiler = mem::replace(&mut self.compiler, enclosing);
                compiler.function
            }
            None => panic!("Didn't find an enclosing compiler"),
        }
    }

    fn function(&mut self, kind: FunctionType) {
        self.push_compiler(kind);
        self.begin_scope();
        self.consume(TokenType::LeftParen, "Expect '(' after function name.");
        if !self.check(TokenType::RightParen) {
            loop {
                self.compiler.function.arity += 1;
                if self.compiler.function.arity > 255 {
                    self.error_at_current("Can't have more than 255 parameters.");
                }
                let param = self.parse_variable("Expect parameter name.");
                self.define_variable(param);
                if !self.matches(TokenType::Comma) {
                    break;
                }
            }
        }
        self.consume(TokenType::RightParen, "Expect ')' after parameters.");
        self.consume(TokenType::LeftBrace, "Expect '{' before function body.");
        self.block();
        let function = self.pop_compiler();
        let fn_id = self.allocator.alloc(function);

        let index = self.make_constant(Value::Function(fn_id));
        self.emit(Instruction::Closure(index));
    }

    fn method(&mut self) {
        self.consume(TokenType::Identifier, "Expect method name.");
        let constant = self.identifier_constant(self.previous);
        let function_type = if self.previous.lexeme == "init" {
            FunctionType::Initializer
        } else {
            FunctionType::Method
        };
        self.function(function_type);
        self.emit(Instruction::Method(constant));
    }

    fn var_declaration(&mut self) {
        let index = self.parse_variable("Expect variable name.");
        if self.matches(TokenType::Equal) {
            self.expression();
        } else {
            self.emit(Instruction::Nil);
        }
        self.consume(
            TokenType::Semicolon,
            "Expect ';' after variable declaration.",
        );
        self.define_variable(index);
    }

    fn define_variable(&mut self, index: u8) {
        if self.compiler.scope_depth > 0 {
            self.mark_initialized();
            return;
        }
        self.emit(Instruction::DefineGlobal(index));
    }

    fn mark_initialized(&mut self) {
        if self.compiler.scope_depth == 0 {
            return;
        }
        let last_local = self.compiler.locals.last_mut().unwrap();
        last_local.depth = self.compiler.scope_depth;
    }

    fn statement(&mut self) {
        if self.matches(TokenType::Print) {
            self.print_statement();
        } else if self.matches(TokenType::If) {
            self.if_statement();
        } else if self.matches(TokenType::Return) {
            self.return_statement();
        } else if self.matches(TokenType::While) {
            self.while_statement();
        } else if self.matches(TokenType::For) {
            self.for_statement();
        } else if self.matches(TokenType::LeftBrace) {
            self.begin_scope();
            self.block();
            self.end_scope();
        } else {
            self.expression_statement();
        }
    }

    fn return_statement(&mut self) {
        if let FunctionType::Script = self.compiler.function_type {
            self.error("Can't return from top-level code.");
        }
        if self.matches(TokenType::Semicolon) {
            self.emit_return();
        } else {
            if let FunctionType::Initializer = self.compiler.function_type {
                self.error("Can't return a value from an initializer.");
            }
            self.expression();
            self.consume(TokenType::Semicolon, "Expect ';' after return value.");
            self.emit(Instruction::Return);
        }
    }

    fn if_statement(&mut self) {
        self.consume(TokenType::LeftParen, "Expect '(' after 'if'.");
        self.expression();
        self.consume(TokenType::RightParen, "Expect ')' after condition.");
        let then_jump = self.emit(Instruction::JumpIfFalse(0xffff));
        self.emit(Instruction::Pop);
        self.statement();
        let else_jump = self.emit(Instruction::Jump(0xffff));
        self.patch_jump(then_jump);
        self.emit(Instruction::Pop);
        if self.matches(TokenType::Else) {
            self.statement();
        }
        self.patch_jump(else_jump);
    }

    fn while_statement(&mut self) {
        let loop_start = self.start_loop();
        self.consume(TokenType::LeftParen, "Expect '(' after 'while'.");
        self.expression();
        self.consume(TokenType::RightParen, "Expect ')' after condition.");
        let exit_jump = self.emit(Instruction::JumpIfFalse(0xffff));
        self.emit(Instruction::Pop);
        self.statement();
        self.emit_loop(loop_start);
        self.patch_jump(exit_jump);
        self.emit(Instruction::Pop);
    }

    fn for_statement(&mut self) {
        self.begin_scope();
        self.consume(TokenType::LeftParen, "Expect '(' after 'for'.");

        // Initializer
        if self.matches(TokenType::Semicolon) {
            // no initializer
        } else if self.matches(TokenType::Var) {
            self.var_declaration();
        } else {
            self.expression_statement();
        }
        let mut loop_start = self.start_loop();

        // Condition
        let mut exit_jump = Option::None;
        if !self.matches(TokenType::Semicolon) {
            self.expression();
            self.consume(TokenType::Semicolon, "Expect ';' after loop condition.");
            let jump = self.emit(Instruction::JumpIfFalse(0xffff));
            exit_jump = Option::from(jump);
            self.emit(Instruction::Pop);
        }

        // Increment
        if !self.matches(TokenType::RightParen) {
            let body_jump = self.emit(Instruction::Jump(0xffff));
            let increment_start = self.start_loop();
            self.expression();
            self.emit(Instruction::Pop);
            self.consume(TokenType::RightParen, "Expect ')' after for clauses.");
            self.emit_loop(loop_start);
            loop_start = increment_start;
            self.patch_jump(body_jump);
        }
        self.statement();
        self.emit_loop(loop_start);
        if let Option::Some(exit_jump) = exit_jump {
            self.patch_jump(exit_jump);
            self.emit(Instruction::Pop);
        }
        self.end_scope();
    }

    fn begin_scope(&mut self) {
        self.compiler.scope_depth += 1;
    }

    fn end_scope(&mut self) {
        self.compiler.scope_depth -= 1;
        for i in (0..self.compiler.locals.len()).rev() {
            if self.compiler.locals[i].depth > self.compiler.scope_depth {
                if self.compiler.locals[i].is_captured {
                    self.emit(Instruction::CloseUpvalue);
                } else {
                    self.emit(Instruction::Pop);
                }
                self.compiler.locals.pop();
            }
        }
    }

    fn block(&mut self) {
        while !self.check(TokenType::RightBrace) && !self.check(TokenType::Eof) {
            self.declaration();
        }
        self.consume(TokenType::RightBrace, "Expect '}' after block.");
    }

    fn print_statement(&mut self) {
        self.expression();
        self.consume(TokenType::Semicolon, "Expect ';' after value.");
        self.emit(Instruction::Print);
    }

    fn number(&mut self, _can_assing: bool) {
        let value: f64 = self
            .previous
            .lexeme
            .parse()
            .expect("Parsed value is not a double");
        self.emit_constant(Value::Number(value));
    }

    fn string(&mut self, _can_assing: bool) {
        let lexeme = self.previous.lexeme;
        let value = &lexeme[1..(lexeme.len() - 1)];
        let s = self.allocator.intern(value.to_owned());
        self.emit_constant(Value::String(s));
    }

    fn literal(&mut self, _can_assing: bool) {
        match self.previous.kind {
            TokenType::False => self.emit(Instruction::False),
            TokenType::True => self.emit(Instruction::True),
            TokenType::Nil => self.emit(Instruction::Nil),
            _ => panic!("Unreachable literal"),
        };
    }

    fn variable(&mut self, can_assing: bool) {
        self.named_variable(self.previous, can_assing);
    }

    fn super_(&mut self, _can_assign: bool) {
        if let Some(current_class) = self.class_compiler.as_ref() {
            if !current_class.has_superclass {
                self.error("Can't use 'super' in a class with no superclass.");
            }
        } else {
            self.error("Can't use 'super' outside of a class.");
        }
        self.consume(TokenType::Dot, "Expect '.' after 'super'.");
        self.consume(TokenType::Identifier, "Expect superclass method name.");
        let name = self.identifier_constant(self.previous);
        self.named_variable(Token::synthetic("this"), false);

        if self.matches(TokenType::LeftParen) {
            let arg_count = self.argument_list();
            self.named_variable(Token::synthetic("super"), false);
            self.emit(Instruction::SuperInvoke((name, arg_count)));
        } else {
            self.named_variable(Token::synthetic("super"), false);
            self.emit(Instruction::GetSuper(name));
        }
    }

    fn this(&mut self, _can_assign: bool) {
        if self.class_compiler.is_none() {
            self.error("Can't use 'this' outside of a class.");
            return;
        }
        self.variable(false);
    }

    fn named_variable(&mut self, name: Token, can_assing: bool) {
        let get_op;
        let set_op;
        if let Some(arg) = self.resolve_local(name) {
            get_op = Instruction::GetLocal(arg);
            set_op = Instruction::SetLocal(arg);
        } else if let Some(arg) = self.resolve_upvalue(name) {
            get_op = Instruction::GetUpvalue(arg);
            set_op = Instruction::SetUpvalue(arg);
        } else {
            let index = self.identifier_constant(name);
            get_op = Instruction::GetGlobal(index);
            set_op = Instruction::SetGlobal(index);
        }

        if can_assing && self.matches(TokenType::Equal) {
            self.expression();
            self.emit(set_op);
        } else {
            self.emit(get_op);
        }
    }

    fn resolve_local(&mut self, name: Token) -> Option<u8> {
        let result = self.compiler.resolve_local(name, &mut self.resolver_errors);
        while let Some(e) = self.resolver_errors.pop() {
            self.error(e);
        }
        result
    }

    fn resolve_upvalue(&mut self, name: Token) -> Option<u8> {
        let result = self
            .compiler
            .resolve_upvalue(name, &mut self.resolver_errors);
        while let Some(e) = self.resolver_errors.pop() {
            self.error(e);
        }
        result
    }

    fn call(&mut self, _can_assing: bool) {
        let arg_count = self.argument_list();
        self.emit(Instruction::Call(arg_count));
    }

    fn dot(&mut self, can_assign: bool) {
        self.consume(TokenType::Identifier, "Expect property name after '.'.");
        let name = self.identifier_constant(self.previous);
        if can_assign && self.matches(TokenType::Equal) {
            self.expression();
            self.emit(Instruction::SetProperty(name));
        } else if self.matches(TokenType::LeftParen) {
            let arg_count = self.argument_list();
            self.emit(Instruction::Invoke((name, arg_count)));
        } else {
            self.emit(Instruction::GetProperty(name));
        }
    }

    fn argument_list(&mut self) -> u8 {
        let mut count: usize = 0;
        if !self.check(TokenType::RightParen) {
            loop {
                self.expression();

                if count == 255 {
                    self.error("Can't have more than 255 arguments.");
                }

                count += 1;
                if !self.matches(TokenType::Comma) {
                    break;
                }
            }
        }
        self.consume(TokenType::RightParen, "Expect ')' after arguments.");
        count as u8
    }

    fn grouping(&mut self, _can_assing: bool) {
        self.expression();
        self.consume(TokenType::RightParen, "Expect ')' after expression.");
    }

    fn unary(&mut self, _can_assing: bool) {
        let operator = self.previous.kind;
        self.parse_precedence(Precedence::Unary);
        match operator {
            TokenType::Bang => self.emit(Instruction::Not),
            TokenType::Minus => self.emit(Instruction::Negate),
            _ => panic!("Invalid unary operator"),
        };
    }

    fn binary(&mut self, _can_assing: bool) {
        let operator = self.previous.kind;
        let rule = self.get_rule(operator);
        self.parse_precedence(rule.precedence.next());
        match operator {
            TokenType::Plus => self.emit(Instruction::Add),
            TokenType::Minus => self.emit(Instruction::Substract),
            TokenType::Star => self.emit(Instruction::Multiply),
            TokenType::Slash => self.emit(Instruction::Divide),
            TokenType::BangEqual => self.emit_two(Instruction::Equal, Instruction::Not),
            TokenType::EqualEqual => self.emit(Instruction::Equal),
            TokenType::Greater => self.emit(Instruction::Greater),
            TokenType::GreaterEqual => self.emit_two(Instruction::Less, Instruction::Not),
            TokenType::Less => self.emit(Instruction::Less),
            TokenType::LessEqual => self.emit_two(Instruction::Greater, Instruction::Not),

            _ => panic!("Invalid unary operator"),
        };
    }

    fn and_op(&mut self, _can_assing: bool) {
        let false_jump = self.emit(Instruction::JumpIfFalse(0xffff));
        self.emit(Instruction::Pop);
        self.parse_precedence(Precedence::And);
        self.patch_jump(false_jump);
    }

    fn or_op(&mut self, _can_assing: bool) {
        let false_jump = self.emit(Instruction::JumpIfFalse(0xffff));
        let true_jump = self.emit(Instruction::Jump(0xffff));
        self.patch_jump(false_jump);
        self.emit(Instruction::Pop);
        self.parse_precedence(Precedence::Or);
        self.patch_jump(true_jump);
    }

    fn parse_precedence(&mut self, precedence: Precedence) {
        self.advance();
        let prefix_rule = self.get_rule(self.previous.kind).prefix;

        let prefix_rule = match prefix_rule {
            Some(rule) => rule,
            None => {
                self.error("Expect expression.");
                return;
            }
        };

        let can_assign = precedence <= Precedence::Assignment;
        prefix_rule(self, can_assign);

        while self.is_lower_precedence(precedence) {
            self.advance();
            let infix_rule = self.get_rule(self.previous.kind).infix.unwrap();
            infix_rule(self, can_assign);
        }

        if can_assign && self.matches(TokenType::Equal) {
            self.error("Invalid assignment target.");
        }
    }

    fn parse_variable(&mut self, msg: &str) -> u8 {
        self.consume(TokenType::Identifier, msg);

        self.declare_variable();
        if self.compiler.scope_depth > 0 {
            return 0;
        }

        self.identifier_constant(self.previous)
    }

    fn identifier_constant(&mut self, token: Token) -> u8 {
        let identifier = self.allocator.intern(token.lexeme.to_owned());
        let value = Value::String(identifier);
        self.make_constant(value)
    }

    fn declare_variable(&mut self) {
        // Global variables are implicitly declared
        if self.compiler.scope_depth == 0 {
            return;
        }
        let name = self.previous;
        if self.compiler.is_local_declared(name) {
            self.error("Already variable with this name in this scope.");
        }
        self.add_local(name);
    }

    fn add_local(&mut self, token: Token<'sourcecode>) {
        if self.compiler.locals.len() == Compiler::LOCAL_COUNT {
            self.error("Too many local variables in function.");
            return;
        }
        let local = Local::new(token, -1);
        self.compiler.locals.push(local);
    }

    fn is_lower_precedence(&self, precedence: Precedence) -> bool {
        let current_precedence = self.get_rule(self.current.kind).precedence;
        precedence <= current_precedence
    }

    fn consume(&mut self, expected: TokenType, msg: &str) {
        if self.current.kind == expected {
            self.advance();
            return;
        }

        self.error_at_current(msg);
    }

    fn advance(&mut self) {
        self.previous = self.current;

        loop {
            self.current = self.scanner.scan_token();
            if self.current.kind == TokenType::Error {
                self.error_at_current(self.current.lexeme);
            } else {
                break;
            }
        }
    }

    fn matches(&mut self, kind: TokenType) -> bool {
        if !self.check(kind) {
            false
        } else {
            self.advance();
            true
        }
    }

    fn check(&self, kind: TokenType) -> bool {
        self.current.kind == kind
    }

    fn error_at_current(&mut self, msg: &str) {
        self.error_at(self.current, msg)
    }

    fn error(&mut self, msg: &str) {
        self.error_at(self.previous, msg)
    }

    fn error_at(&mut self, token: Token, msg: &str) {
        if self.panic_mode {
            return;
        }

        self.had_error = true;
        self.panic_mode = true;
        eprint!("[line {}] Error", token.line);
        match token.kind {
            TokenType::Eof => eprint!(" at end"),
            TokenType::Error => (),
            _ => eprint!(" at '{}'", token.lexeme),
        };
        eprintln!(": {}", msg);
    }

    fn synchronize(&mut self) {
        self.panic_mode = false;

        while self.previous.kind != TokenType::Eof {
            if self.previous.kind == TokenType::Semicolon {
                return;
            }

            match self.current.kind {
                TokenType::Class
                | TokenType::Fun
                | TokenType::Var
                | TokenType::For
                | TokenType::If
                | TokenType::While
                | TokenType::Print
                | TokenType::Return => return,
                _ => (),
            }

            self.advance()
        }
    }

    fn emit(&mut self, instruction: Instruction) -> usize {
        self.compiler
            .function
            .chunk
            .write(instruction, self.previous.line)
    }

    fn emit_two(&mut self, i1: Instruction, i2: Instruction) -> usize {
        self.compiler.function.chunk.write(i1, self.previous.line);
        self.compiler.function.chunk.write(i2, self.previous.line)
    }

    fn emit_return(&mut self) -> usize {
        match self.compiler.function_type {
            FunctionType::Initializer => self.emit(Instruction::GetLocal(0)),
            _ => self.emit(Instruction::Nil),
        };
        self.emit(Instruction::Return)
    }

    fn start_loop(&self) -> usize {
        self.compiler.function.chunk.code.len()
    }

    fn emit_loop(&mut self, start_pos: usize) {
        let offset = self.compiler.function.chunk.code.len() - start_pos;
        let offset = match u16::try_from(offset) {
            Ok(o) => o,
            Err(_) => {
                self.error("Loop body too large.");
                0xffff
            }
        };
        self.emit(Instruction::Loop(offset));
    }

    fn patch_jump(&mut self, pos: usize) {
        let offset = self.compiler.function.chunk.code.len() - 1 - pos;
        let offset = match u16::try_from(offset) {
            Ok(offset) => offset,
            Err(_) => {
                self.error("Too much code to jump over.");
                0xfff
            }
        };

        match self.compiler.function.chunk.code[pos] {
            Instruction::JumpIfFalse(ref mut o) => *o = offset,
            Instruction::Jump(ref mut o) => *o = offset,
            _ => panic!("Instruction at position is not jump"),
        }
    }

    fn make_constant(&mut self, value: Value) -> u8 {
        let index = self.compiler.function.chunk.add_constant(value);
        match u8::try_from(index) {
            Ok(index) => index,
            Err(_) => {
                self.error("Too many constants in one chunk.");
                0
            }
        }
    }

    fn emit_constant(&mut self, value: Value) {
        let index = self.make_constant(value);
        self.emit(Instruction::Constant(index));
    }

    fn get_rule(&self, kind: TokenType) -> ParseRule<'sourcecode> {
        self.rules.get(&kind).cloned().unwrap()
    }
}

pub fn compile(code: &str, allocator: &mut Allocator) -> Result<Reference<LoxFunction>, LoxError> {
    let parser = Parser::new(code, allocator);
    Ok(parser.compile()?)
}
