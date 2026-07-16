//! Self-contained mathematical expression compiler and evaluator for computed
//! channel formulas.
//!
//! This replaces the unmaintained `meval` crate (last release 2017, depends on
//! `nom` 1.2.4 which will be rejected by a future version of Rust) while
//! preserving meval's exact grammar and semantics:
//!
//! - Operators: `+ - * / % ^` with meval's precedence table
//!   (`+ -` = 1 left, `* / %` = 2 left, unary `+ -` = 3, `^` = 4 **right**)
//!   so `-2^2 == -4` and `2^3^2 == 512`, matching standard math convention.
//! - Numbers: `2`, `2.`, `2.5`, `0.125e9`, `20.5E-3` (no leading-dot floats).
//! - Identifiers: start with a letter or `_`, continue with letters, digits
//!   or `_`. Unicode letters are accepted (a superset of meval, which was
//!   ASCII-only) so sanitized international channel names work.
//! - Functions: meval's built-in set, plus `log2`, `log10`, `trunc`, `fract`
//!   and 2-arg `pow` (all names were already reserved in formulas). `min` and
//!   `max` are variadic with at least one argument. `log` is intentionally
//!   not defined (it was not defined in meval either, and its base would be
//!   ambiguous); the error suggests `ln`/`log10`.
//! - Constants: `pi` and `e` (meval), plus `tau` and `phi` (already reserved).
//!
//! Formulas are compiled once to an RPN instruction list with variables
//! resolved to slots, then evaluated per record against a value slice with a
//! reusable stack — no per-record parsing, hashing or allocation.

/// Binary operators, with meval's precedence and associativity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
}

impl BinOp {
    fn prec(self) -> u32 {
        match self {
            BinOp::Add | BinOp::Sub => 1,
            BinOp::Mul | BinOp::Div | BinOp::Rem => 2,
            BinOp::Pow => 4,
        }
    }

    fn right_assoc(self) -> bool {
        matches!(self, BinOp::Pow)
    }

    fn apply(self, a: f64, b: f64) -> f64 {
        match self {
            BinOp::Add => a + b,
            BinOp::Sub => a - b,
            BinOp::Mul => a * b,
            BinOp::Div => a / b,
            BinOp::Rem => a % b,
            BinOp::Pow => a.powf(b),
        }
    }
}

/// Unary operator precedence: binds tighter than `* /` but looser than `^`,
/// exactly like meval (so `-2^2` is `-(2^2)`).
const UNARY_PREC: u32 = 3;

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Var(String),
    /// Function name; the argument count is filled in during RPN conversion.
    Func(String, usize),
    UnaryPlus,
    UnaryMinus,
    Binary(BinOp),
    LParen,
    RParen,
    Comma,
}

/// A compiled instruction operating on the evaluation stack.
#[derive(Debug, Clone)]
enum Instr {
    Const(f64),
    /// Push the value of variable slot `n`.
    Var(usize),
    Bin(BinOp),
    Neg,
    Func1(fn(f64) -> f64),
    Func2(fn(f64, f64) -> f64),
    /// Fold the top `n` stack values with `f64::min`.
    MinN(usize),
    /// Fold the top `n` stack values with `f64::max`.
    MaxN(usize),
}

/// A formula compiled to RPN with variables resolved to slot indices.
#[derive(Debug, Clone)]
pub struct CompiledExpr {
    instrs: Vec<Instr>,
    var_names: Vec<String>,
    max_stack: usize,
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Length in bytes of the identifier at the start of `s`.
fn ident_len(s: &str) -> usize {
    let mut chars = s.char_indices();
    match chars.next() {
        Some((_, c)) if is_ident_start(c) => {}
        _ => return 0,
    }
    for (i, c) in chars {
        if !is_ident_continue(c) {
            return i;
        }
    }
    s.len()
}

/// Parse a number literal at the start of `s`, returning (value, length).
///
/// Grammar (same as meval): `digit+ ('.' digit*)? ([eE] [+-]? digit+)?`.
/// A trailing `e`/`E` without a valid exponent makes the whole literal
/// invalid (meval behaves the same way).
fn number_len(s: &str) -> Result<usize, String> {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    debug_assert!(i > 0, "number_len called on non-digit start");
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
    }
    if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
        let mut j = i + 1;
        if j < bytes.len() && (bytes[j] == b'+' || bytes[j] == b'-') {
            j += 1;
        }
        let exp_digits_start = j;
        while j < bytes.len() && bytes[j].is_ascii_digit() {
            j += 1;
        }
        if j == exp_digits_start {
            return Err(format!(
                "invalid number literal '{}'",
                &s[..(j).min(s.len())]
            ));
        }
        i = j;
    }
    Ok(i)
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TokState {
    /// Expecting an operand: number, var, function, unary op or '('.
    LExpr,
    /// Expecting an operator: binary op, ')' or ','.
    AfterRExpr,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ParenKind {
    Subexpr,
    Func,
}

/// Tokenize a formula using the same state machine as meval's tokenizer.
fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let mut state = TokState::LExpr;
    let mut paren_stack: Vec<ParenKind> = Vec::new();
    let mut tokens = Vec::new();

    let mut rest = input.trim_start();
    while !rest.is_empty() {
        let c = rest.chars().next().unwrap();
        let (token, len) = match state {
            TokState::LExpr => {
                if c.is_ascii_digit() {
                    let len = number_len(rest)?;
                    let value: f64 = rest[..len]
                        .parse()
                        .map_err(|_| format!("invalid number literal '{}'", &rest[..len]))?;
                    (Token::Number(value), len)
                } else if is_ident_start(c) {
                    let len = ident_len(rest);
                    let name = &rest[..len];
                    // A '(' after optional whitespace makes this a function call.
                    let after = rest[len..].trim_start();
                    if let Some(stripped) = after.strip_prefix('(') {
                        let consumed = rest.len() - stripped.len();
                        (Token::Func(name.to_string(), 0), consumed)
                    } else {
                        (Token::Var(name.to_string()), len)
                    }
                } else {
                    match c {
                        '+' => (Token::UnaryPlus, 1),
                        '-' => (Token::UnaryMinus, 1),
                        '(' => (Token::LParen, 1),
                        _ => return Err(format!("unexpected character '{c}'")),
                    }
                }
            }
            TokState::AfterRExpr => match c {
                '+' => (Token::Binary(BinOp::Add), 1),
                '-' => (Token::Binary(BinOp::Sub), 1),
                '*' => (Token::Binary(BinOp::Mul), 1),
                '/' => (Token::Binary(BinOp::Div), 1),
                '%' => (Token::Binary(BinOp::Rem), 1),
                '^' => (Token::Binary(BinOp::Pow), 1),
                ')' if !paren_stack.is_empty() => (Token::RParen, 1),
                ',' if paren_stack.last() == Some(&ParenKind::Func) => (Token::Comma, 1),
                _ => return Err(format!("unexpected character '{c}'")),
            },
        };

        match token {
            Token::LParen => paren_stack.push(ParenKind::Subexpr),
            Token::Func(..) => paren_stack.push(ParenKind::Func),
            Token::RParen => {
                paren_stack.pop();
                state = TokState::AfterRExpr;
            }
            Token::Var(_) | Token::Number(_) => state = TokState::AfterRExpr,
            Token::Binary(_) | Token::Comma => state = TokState::LExpr,
            Token::UnaryPlus | Token::UnaryMinus => {}
        }

        tokens.push(token);
        rest = rest[len..].trim_start();
    }

    if state == TokState::LExpr {
        return Err("missing operand".to_string());
    }
    if !paren_stack.is_empty() {
        return Err("missing closing parenthesis".to_string());
    }
    Ok(tokens)
}

/// Convert infix tokens to RPN using meval's shunting-yard rules.
fn to_rpn(tokens: Vec<Token>) -> Result<Vec<Token>, String> {
    let mut output: Vec<Token> = Vec::with_capacity(tokens.len());
    let mut stack: Vec<Token> = Vec::new();

    // Precedence of a stacked operator token (operands never end up here).
    fn stack_prec(token: &Token) -> Option<u32> {
        match token {
            Token::Binary(op) => Some(op.prec()),
            Token::UnaryPlus | Token::UnaryMinus => Some(UNARY_PREC),
            _ => None,
        }
    }

    for token in tokens {
        match token {
            Token::Number(_) | Token::Var(_) => output.push(token),
            Token::UnaryPlus | Token::UnaryMinus => stack.push(token),
            Token::Binary(op) => {
                while let Some(top) = stack.last() {
                    let Some(top_prec) = stack_prec(top) else {
                        break;
                    };
                    let pops = if op.right_assoc() {
                        op.prec() < top_prec
                    } else {
                        op.prec() <= top_prec
                    };
                    if pops {
                        output.push(stack.pop().unwrap());
                    } else {
                        break;
                    }
                }
                stack.push(token);
            }
            Token::LParen | Token::Func(..) => stack.push(token),
            Token::RParen => {
                let mut found = false;
                while let Some(top) = stack.pop() {
                    match top {
                        Token::LParen => {
                            found = true;
                            break;
                        }
                        Token::Func(name, nargs) => {
                            found = true;
                            output.push(Token::Func(name, nargs + 1));
                            break;
                        }
                        other => output.push(other),
                    }
                }
                if !found {
                    return Err("mismatched closing parenthesis".to_string());
                }
            }
            Token::Comma => {
                let mut found = false;
                while let Some(top) = stack.pop() {
                    match top {
                        Token::LParen => return Err("unexpected comma".to_string()),
                        Token::Func(name, nargs) => {
                            found = true;
                            stack.push(Token::Func(name, nargs + 1));
                            break;
                        }
                        other => output.push(other),
                    }
                }
                if !found {
                    return Err("unexpected comma".to_string());
                }
            }
        }
    }

    while let Some(top) = stack.pop() {
        match top {
            Token::Binary(_) | Token::UnaryPlus | Token::UnaryMinus => output.push(top),
            _ => return Err("missing closing parenthesis".to_string()),
        }
    }

    Ok(output)
}

fn func1(name: &str) -> Option<fn(f64) -> f64> {
    Some(match name {
        "sqrt" => f64::sqrt,
        "exp" => f64::exp,
        "ln" => f64::ln,
        "log2" => f64::log2,
        "log10" => f64::log10,
        "abs" => f64::abs,
        "sin" => f64::sin,
        "cos" => f64::cos,
        "tan" => f64::tan,
        "asin" => f64::asin,
        "acos" => f64::acos,
        "atan" => f64::atan,
        "sinh" => f64::sinh,
        "cosh" => f64::cosh,
        "tanh" => f64::tanh,
        "asinh" => f64::asinh,
        "acosh" => f64::acosh,
        "atanh" => f64::atanh,
        "floor" => f64::floor,
        "ceil" => f64::ceil,
        "round" => f64::round,
        "trunc" => f64::trunc,
        "fract" => f64::fract,
        "signum" => f64::signum,
        _ => return None,
    })
}

fn func2(name: &str) -> Option<fn(f64, f64) -> f64> {
    Some(match name {
        "atan2" => f64::atan2,
        "pow" => f64::powf,
        _ => return None,
    })
}

fn constant(name: &str) -> Option<f64> {
    Some(match name {
        "pi" => std::f64::consts::PI,
        "e" => std::f64::consts::E,
        "tau" => std::f64::consts::TAU,
        "phi" => 1.618_033_988_749_895_f64,
        _ => return None,
    })
}

impl CompiledExpr {
    /// Parse and compile a formula. Every identifier that is not a function
    /// or constant becomes a variable slot in `var_names` (first-appearance
    /// order).
    pub fn parse(input: &str) -> Result<Self, String> {
        let rpn = to_rpn(tokenize(input)?)?;

        let mut instrs = Vec::with_capacity(rpn.len());
        let mut var_names: Vec<String> = Vec::new();
        let mut slot_by_name: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for token in rpn {
            let instr = match token {
                Token::Number(value) => Instr::Const(value),
                Token::Var(name) => {
                    if let Some(value) = constant(&name) {
                        Instr::Const(value)
                    } else if let Some(&slot) = slot_by_name.get(&name) {
                        Instr::Var(slot)
                    } else {
                        let slot = var_names.len();
                        var_names.push(name.clone());
                        slot_by_name.insert(name, slot);
                        Instr::Var(slot)
                    }
                }
                Token::Func(name, nargs) => match name.as_str() {
                    "min" => {
                        if nargs == 0 {
                            return Err("function 'min' needs at least 1 argument".to_string());
                        }
                        Instr::MinN(nargs)
                    }
                    "max" => {
                        if nargs == 0 {
                            return Err("function 'max' needs at least 1 argument".to_string());
                        }
                        Instr::MaxN(nargs)
                    }
                    _ => {
                        if let Some(f) = func1(&name) {
                            if nargs != 1 {
                                return Err(format!(
                                    "function '{name}' expects 1 argument, got {nargs}"
                                ));
                            }
                            Instr::Func1(f)
                        } else if let Some(f) = func2(&name) {
                            if nargs != 2 {
                                return Err(format!(
                                    "function '{name}' expects 2 arguments, got {nargs}"
                                ));
                            }
                            Instr::Func2(f)
                        } else if name == "log" {
                            return Err(
                                "unknown function 'log' (use 'ln' for natural log or 'log10')"
                                    .to_string(),
                            );
                        } else {
                            return Err(format!("unknown function '{name}'"));
                        }
                    }
                },
                Token::Binary(op) => Instr::Bin(op),
                Token::UnaryMinus => Instr::Neg,
                // Unary plus is the identity; drop it.
                Token::UnaryPlus => continue,
                Token::LParen | Token::RParen | Token::Comma => {
                    unreachable!("parens/commas never appear in RPN output")
                }
            };
            instrs.push(instr);
        }

        // Verify stack balance and compute the maximum stack depth so that
        // `eval` can pre-reserve and never reallocate.
        let mut depth: isize = 0;
        let mut max_stack: isize = 0;
        for instr in &instrs {
            let delta = match instr {
                Instr::Const(_) | Instr::Var(_) => 1,
                Instr::Neg | Instr::Func1(_) => 0,
                Instr::Bin(_) | Instr::Func2(_) => -1,
                Instr::MinN(n) | Instr::MaxN(n) => 1 - (*n as isize),
            };
            depth += delta;
            if depth <= 0 {
                return Err("missing operand".to_string());
            }
            max_stack = max_stack.max(depth);
        }
        if depth != 1 {
            return Err("too many operands".to_string());
        }

        Ok(CompiledExpr {
            instrs,
            var_names,
            max_stack: max_stack as usize,
        })
    }

    /// Variable slot names in slot order; `eval` expects values in this order.
    pub fn var_names(&self) -> &[String] {
        &self.var_names
    }

    /// Evaluate against variable slot values, reusing `stack` as scratch
    /// space so repeated evaluation does not allocate.
    pub fn eval_with_stack(&self, vars: &[f64], stack: &mut Vec<f64>) -> f64 {
        debug_assert_eq!(vars.len(), self.var_names.len());
        stack.clear();
        stack.reserve(self.max_stack);
        for instr in &self.instrs {
            match instr {
                Instr::Const(value) => stack.push(*value),
                Instr::Var(slot) => stack.push(vars.get(*slot).copied().unwrap_or(0.0)),
                Instr::Neg => {
                    let top = stack.last_mut().expect("validated at compile time");
                    *top = -*top;
                }
                Instr::Bin(op) => {
                    let b = stack.pop().expect("validated at compile time");
                    let a = stack.last_mut().expect("validated at compile time");
                    *a = op.apply(*a, b);
                }
                Instr::Func1(f) => {
                    let top = stack.last_mut().expect("validated at compile time");
                    *top = f(*top);
                }
                Instr::Func2(f) => {
                    let b = stack.pop().expect("validated at compile time");
                    let a = stack.last_mut().expect("validated at compile time");
                    *a = f(*a, b);
                }
                Instr::MinN(n) => {
                    let base = stack.len() - n;
                    let folded = stack[base..].iter().copied().fold(f64::INFINITY, f64::min);
                    stack.truncate(base);
                    stack.push(folded);
                }
                Instr::MaxN(n) => {
                    let base = stack.len() - n;
                    let folded = stack[base..]
                        .iter()
                        .copied()
                        .fold(f64::NEG_INFINITY, f64::max);
                    stack.truncate(base);
                    stack.push(folded);
                }
            }
        }
        stack.pop().expect("validated at compile time")
    }

    /// Convenience single-shot evaluation (allocates a fresh stack).
    #[cfg(test)]
    pub fn eval(&self, vars: &[f64]) -> f64 {
        let mut stack = Vec::with_capacity(self.max_stack);
        self.eval_with_stack(vars, &mut stack)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(src: &str) -> f64 {
        CompiledExpr::parse(src).unwrap().eval(&[])
    }

    fn eval_vars(src: &str, vars: &[(&str, f64)]) -> f64 {
        let compiled = CompiledExpr::parse(src).unwrap();
        let vals: Vec<f64> = compiled
            .var_names()
            .iter()
            .map(|n| {
                vars.iter()
                    .find(|(name, _)| name == n)
                    .map(|(_, v)| *v)
                    .unwrap_or_else(|| panic!("no value for variable '{n}'"))
            })
            .collect();
        compiled.eval(&vals)
    }

    /// Golden values computed with meval 0.2 before it was removed, pinning
    /// this engine to meval's exact semantics (precedence, associativity,
    /// unary binding, `%`, variadic min/max, constants, number formats).
    /// The engine was additionally verified against meval with 200k
    /// differentially-fuzzed random expressions at replacement time.
    #[test]
    fn test_meval_golden_values() {
        let vars = [("A", 3.5_f64), ("B", -2.25_f64), ("C", 0.75_f64)];
        let cases: &[(&str, f64)] = &[
            ("-2^2", -4.0),
            ("2^3^2", 512.0),
            ("-(2+1)^2", -9.0),
            ("2^-2", 0.25),
            ("A - -B", 1.25),
            ("-A^2 + B", -14.5),
            ("A * -B^2", -17.71875),
            ("A % B * C", 0.9375),
            ("A / B / C", -2.074074074074074),
            ("A - B - C", 5.0),
            ("2 ^ A ^ C", 5.892527391914566),
            ("min(A, B, C, 2)", -2.25),
            ("max(A, min(B, C), abs(B))", 3.5),
            ("atan2(A, B) + atan2(B, A)", 1.5707963267948966),
            ("sin(A)^2 + cos(A)^2", 1.0),
            ("sqrt(abs(B)) * exp(C) - ln(A)", 1.9227370564236441),
            ("tanh(C) + asinh(A) - atan(B)", 3.7534414212526066),
            ("floor(A) + ceil(B) + round(C) + signum(B)", 1.0),
            ("(A + B) * (A - B) / (C + 1)", 4.107142857142857),
            ("pi * A^2 + e^C", 40.60151002308764),
            ("1e2 + 2.5E-1 * A", 100.875),
            ("17.", 17.0),
            ("0.125e1 ^ 2", 1.5625),
            ("--A + +B", 1.25),
            ("A*(B+C)^2-4/C%3", 5.541666666666667),
            ("min(max(A, B), max(B, C), 1.5)", 0.75),
            ("acosh(A) + sinh(B) % cosh(C)", 1.1177288483706174),
            ("asin(C) * acos(C) / atan(A)", 0.4742167032429436),
        ];
        for (src, expected) in cases {
            let got = eval_vars(src, &vars);
            assert!(
                (got - expected).abs() <= 1e-12 * expected.abs().max(1.0),
                "'{src}': got {got}, expected {expected}"
            );
        }
    }

    #[test]
    fn test_basic_arithmetic() {
        assert_eq!(eval("1 + 2 * 3"), 7.0);
        assert_eq!(eval("(1 + 2) * 3"), 9.0);
        assert_eq!(eval("10 / 4"), 2.5);
        assert_eq!(eval("7 % 3"), 1.0);
        assert_eq!(eval("2 ^ 10"), 1024.0);
    }

    #[test]
    fn test_meval_precedence_semantics() {
        // Unary minus binds looser than `^` (standard math convention).
        assert_eq!(eval("-2^2"), -4.0);
        assert_eq!(eval("-(2+1)^2"), -9.0);
        // `^` is right-associative.
        assert_eq!(eval("2^3^2"), 512.0);
        // Unary minus binds tighter than `*` and `/`.
        assert_eq!(eval("-2 * 3"), -6.0);
        assert_eq!(eval("2 * -3"), -6.0);
        assert_eq!(eval("2^-2"), 0.25);
        assert_eq!(eval("1 - -2"), 3.0);
        assert_eq!(eval("--2"), 2.0);
        assert_eq!(eval("+2"), 2.0);
    }

    #[test]
    fn test_number_formats() {
        assert_eq!(eval("2."), 2.0);
        assert_eq!(eval("2.5"), 2.5);
        assert_eq!(eval("0.125e9"), 0.125e9);
        assert_eq!(eval("20.5E-3"), 20.5e-3);
        assert_eq!(eval("123e+2"), 12300.0);
    }

    #[test]
    fn test_invalid_numbers() {
        assert!(CompiledExpr::parse(".5").is_err()); // no leading-dot floats
        assert!(CompiledExpr::parse("1e").is_err());
        assert!(CompiledExpr::parse("1e+").is_err());
    }

    #[test]
    fn test_functions() {
        assert!((eval("sqrt(16)") - 4.0).abs() < 1e-12);
        assert!((eval("sin(0)") - 0.0).abs() < 1e-12);
        assert!((eval("cos(0)") - 1.0).abs() < 1e-12);
        assert!((eval("abs(-3)") - 3.0).abs() < 1e-12);
        assert!((eval("ln(e)") - 1.0).abs() < 1e-12);
        assert!((eval("log10(100)") - 2.0).abs() < 1e-12);
        assert!((eval("log2(8)") - 3.0).abs() < 1e-12);
        assert!((eval("atan2(1, 1)") - std::f64::consts::FRAC_PI_4).abs() < 1e-12);
        assert!((eval("pow(2, 10)") - 1024.0).abs() < 1e-12);
        assert!((eval("signum(-5)") - -1.0).abs() < 1e-12);
        assert!((eval("trunc(2.7)") - 2.0).abs() < 1e-12);
        assert!((eval("fract(2.75)") - 0.75).abs() < 1e-12);
        // Function name followed by whitespace then '(' is still a call.
        assert!((eval("sqrt (16)") - 4.0).abs() < 1e-12);
    }

    #[test]
    fn test_variadic_min_max() {
        assert_eq!(eval("min(3)"), 3.0);
        assert_eq!(eval("min(3, 1, 2)"), 1.0);
        assert_eq!(eval("max(3, 1, 2, 9, 4)"), 9.0);
        assert_eq!(eval("max(min(3, 1), 2)"), 2.0);
    }

    #[test]
    fn test_constants() {
        assert_eq!(eval("pi"), std::f64::consts::PI);
        assert_eq!(eval("e"), std::f64::consts::E);
        assert_eq!(eval("tau"), std::f64::consts::TAU);
        assert!((eval("phi") - 1.618033988749895).abs() < 1e-12);
    }

    #[test]
    fn test_variables() {
        assert_eq!(eval_vars("X * 2", &[("X", 21.0)]), 42.0);
        assert_eq!(
            eval_vars(
                "RPM_lb__neg_1_rb_ + _mean_RPM",
                &[("RPM_lb__neg_1_rb_", 5.0), ("_mean_RPM", 2.0)]
            ),
            7.0
        );
        // Repeated variables share one slot.
        let compiled = CompiledExpr::parse("X + X * X").unwrap();
        assert_eq!(compiled.var_names(), ["X"]);
        assert_eq!(compiled.eval(&[3.0]), 12.0);
    }

    #[test]
    fn test_var_slot_order_is_first_appearance() {
        let compiled = CompiledExpr::parse("B + A").unwrap();
        assert_eq!(compiled.var_names(), ["B", "A"]);
        assert_eq!(compiled.eval(&[1.0, 10.0]), 11.0);
    }

    #[test]
    fn test_parse_errors() {
        assert!(CompiledExpr::parse("").is_err());
        assert!(CompiledExpr::parse("   ").is_err());
        assert!(CompiledExpr::parse("1 +").is_err());
        assert!(CompiledExpr::parse("+ + +").is_err());
        assert!(CompiledExpr::parse("(1").is_err());
        assert!(CompiledExpr::parse("1)").is_err());
        assert!(CompiledExpr::parse("min()").is_err());
        assert!(CompiledExpr::parse("1, 2").is_err()); // comma outside function
        assert!(CompiledExpr::parse("2 3").is_err()); // no implicit multiplication
        assert!(CompiledExpr::parse("2 (3)").is_err());
        assert!(CompiledExpr::parse("sqrt(1, 2)").is_err()); // wrong arity
        assert!(CompiledExpr::parse("atan2(1)").is_err());
        assert!(CompiledExpr::parse("unknown_func(1)").is_err());
        // 'log' is intentionally undefined; the error should point at ln/log10.
        let err = CompiledExpr::parse("log(10)").unwrap_err();
        assert!(err.contains("ln"), "unexpected error: {err}");
    }

    #[test]
    fn test_float_edge_semantics() {
        assert!(eval("1 / 0").is_infinite());
        assert!(eval("0 / 0").is_nan());
        assert!(eval("sqrt(-1)").is_nan());
        // min/max fold from +/-infinity, ignoring NaN (same as meval).
        assert_eq!(eval("max(0 / 0, 1)"), 1.0);
    }
}
